//! Multi-source object search logic.
//!
//! Search order: loose -> packs (newest first) -> alternates.
//! This matches C git behavior where loose objects are preferred because
//! they may be newer (e.g., during a repack operation).

use git_hash::ObjectId;
use git_object::Object;

use crate::{ObjectDatabase, ObjectInfo, OdbError};

/// Find an object by OID, searching all backends in order.
pub(crate) fn find_object(
    odb: &ObjectDatabase,
    oid: &ObjectId,
) -> Result<Option<Object>, OdbError> {
    // 1. Check loose objects first
    if let Some(obj) = odb.loose.read(oid)? {
        return Ok(Some(obj));
    }

    // 2. Check pack files (newest first, sorted at discovery time)
    //    Use cross-pack resolver so REF_DELTA bases can be found in other
    //    packs or loose objects.
    {
        let packs = odb.packs.read().unwrap();
        for pack in packs.iter() {
            let resolver = |base_oid: &git_hash::ObjectId| -> Option<(git_object::ObjectType, Vec<u8>)> {
                // Search loose objects for the base
                if let Ok(Some(obj)) = odb.loose.read(base_oid) {
                    return Some((obj.object_type(), obj.serialize_content()));
                }
                // Search other packs for the base (avoid infinite recursion by using read_object)
                for other_pack in packs.iter() {
                    if std::ptr::eq(other_pack, pack) {
                        continue;
                    }
                    if let Ok(Some(packed)) = other_pack.read_object(base_oid) {
                        return Some((packed.obj_type, packed.data));
                    }
                }
                None
            };
            match pack.read_object_with_resolver(oid, resolver)? {
                Some(packed) => {
                    let obj = Object::parse_content(packed.obj_type, &packed.data)
                        .map_err(|e| OdbError::Corrupt {
                            oid: *oid,
                            reason: e.to_string(),
                        })?;
                    return Ok(Some(obj));
                }
                None => continue,
            }
        }
    }

    // 3. Check alternates
    for alt in &odb.alternates {
        if let Some(obj) = alt.read(oid)? {
            return Ok(Some(obj));
        }
    }

    Ok(None)
}

/// Find an object header by OID, searching all backends in order.
pub(crate) fn find_header(
    odb: &ObjectDatabase,
    oid: &ObjectId,
) -> Result<Option<ObjectInfo>, OdbError> {
    // 1. Check loose objects first
    if let Some((obj_type, size)) = odb.loose.read_header(oid)? {
        return Ok(Some(ObjectInfo { obj_type, size }));
    }

    // 2. Check pack files
    {
        let packs = odb.packs.read().unwrap();
        for pack in packs.iter() {
            match pack.read_object(oid)? {
                Some(packed) => {
                    return Ok(Some(ObjectInfo {
                        obj_type: packed.obj_type,
                        size: packed.data.len(),
                    }));
                }
                None => continue,
            }
        }
    }

    // 3. Check alternates
    for alt in &odb.alternates {
        if let Some(info) = alt.read_header(oid)? {
            return Ok(Some(info));
        }
    }

    Ok(None)
}

/// Check if an object exists in any backend (fast, no decompression for packs).
pub(crate) fn object_exists(odb: &ObjectDatabase, oid: &ObjectId) -> bool {
    // 1. Check loose
    if odb.loose.contains(oid) {
        return true;
    }

    // 2. Check packs (index lookup only, no decompression)
    {
        let packs = odb.packs.read().unwrap();
        for pack in packs.iter() {
            if pack.contains(oid) {
                return true;
            }
        }
    }

    // 3. Check alternates
    for alt in &odb.alternates {
        if alt.contains(oid) {
            return true;
        }
    }

    false
}
