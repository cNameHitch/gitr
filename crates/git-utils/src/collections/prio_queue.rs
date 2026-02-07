use std::cmp::Ordering;

type Comparator<T> = Box<dyn Fn(&T, &T) -> Ordering>;

/// An entry in the priority queue, tracking insertion order for stability.
#[derive(Debug)]
struct PrioQueueEntry<T> {
    ctr: u64,
    data: T,
}

/// A priority queue that can operate as either a min-heap (with a comparator)
/// or a LIFO stack (without a comparator).
///
/// This mirrors C git's `prio_queue` data structure, which is used for
/// traversing commits in date order. The queue is stable: items that compare
/// equal come out in insertion order.
pub struct PriorityQueue<T> {
    array: Vec<PrioQueueEntry<T>>,
    compare: Option<Comparator<T>>,
    insertion_ctr: u64,
}

impl<T> PriorityQueue<T> {
    /// Create a new priority queue with a comparison function (min-heap mode).
    /// Items that compare as `Less` are extracted first.
    pub fn new(compare: impl Fn(&T, &T) -> Ordering + 'static) -> Self {
        Self {
            array: Vec::new(),
            compare: Some(Box::new(compare)),
            insertion_ctr: 0,
        }
    }

    /// Create a new priority queue in LIFO (stack) mode.
    pub fn new_lifo() -> Self {
        Self {
            array: Vec::new(),
            compare: None,
            insertion_ctr: 0,
        }
    }

    /// Compare two entries, using the comparator and insertion order for stability.
    fn compare_entries(&self, i: usize, j: usize) -> Ordering {
        if let Some(ref cmp) = self.compare {
            let result = cmp(&self.array[i].data, &self.array[j].data);
            if result != Ordering::Equal {
                return result;
            }
            // Tie-break by insertion order (earlier insertions first)
            self.array[i].ctr.cmp(&self.array[j].ctr)
        } else {
            Ordering::Equal
        }
    }

    /// Add an item to the queue.
    pub fn put(&mut self, thing: T) {
        let ctr = self.insertion_ctr;
        self.insertion_ctr += 1;
        self.array.push(PrioQueueEntry { ctr, data: thing });

        if self.compare.is_none() {
            return; // LIFO mode, no heapification needed
        }

        // Bubble up
        let mut ix = self.array.len() - 1;
        while ix > 0 {
            let parent = (ix - 1) / 2;
            if self.compare_entries(parent, ix) != Ordering::Greater {
                break;
            }
            self.array.swap(parent, ix);
            ix = parent;
        }
    }

    /// Extract the highest-priority item (smallest per comparator) or the
    /// most recently added item in LIFO mode.
    pub fn get(&mut self) -> Option<T> {
        if self.array.is_empty() {
            return None;
        }

        if self.compare.is_none() {
            // LIFO mode
            return Some(self.array.pop().unwrap().data);
        }

        // Min-heap mode: take root, replace with last, sift down
        let len = self.array.len();
        if len == 1 {
            return Some(self.array.pop().unwrap().data);
        }

        self.array.swap(0, len - 1);
        let result = self.array.pop().unwrap().data;
        self.sift_down_root();
        Some(result)
    }

    /// Peek at the highest-priority item without removing it.
    pub fn peek(&self) -> Option<&T> {
        if self.array.is_empty() {
            return None;
        }
        if self.compare.is_none() {
            // LIFO: peek at last
            Some(&self.array.last().unwrap().data)
        } else {
            Some(&self.array[0].data)
        }
    }

    /// Sift the root element down to restore heap property.
    fn sift_down_root(&mut self) {
        let mut ix = 0;
        loop {
            let left = ix * 2 + 1;
            if left >= self.array.len() {
                break;
            }
            let right = left + 1;
            let child = if right < self.array.len()
                && self.compare_entries(left, right) != Ordering::Less
            {
                right
            } else {
                left
            };

            if self.compare_entries(ix, child) != Ordering::Greater {
                break;
            }

            self.array.swap(child, ix);
            ix = child;
        }
    }

    /// Reverse the order (only valid for LIFO queues).
    pub fn reverse(&mut self) {
        assert!(
            self.compare.is_none(),
            "reverse() only valid on LIFO queues"
        );
        self.array.reverse();
    }

    /// Get the number of items in the queue.
    pub fn len(&self) -> usize {
        self.array.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.array.is_empty()
    }

    /// Remove all items.
    pub fn clear(&mut self) {
        self.array.clear();
        self.insertion_ctr = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_heap_basic() {
        let mut pq = PriorityQueue::new(|a: &i32, b: &i32| a.cmp(b));
        pq.put(3);
        pq.put(1);
        pq.put(2);

        assert_eq!(pq.get(), Some(1));
        assert_eq!(pq.get(), Some(2));
        assert_eq!(pq.get(), Some(3));
        assert_eq!(pq.get(), None);
    }

    #[test]
    fn max_heap() {
        let mut pq = PriorityQueue::new(|a: &i32, b: &i32| b.cmp(a));
        pq.put(1);
        pq.put(3);
        pq.put(2);

        assert_eq!(pq.get(), Some(3));
        assert_eq!(pq.get(), Some(2));
        assert_eq!(pq.get(), Some(1));
    }

    #[test]
    fn lifo_mode() {
        let mut pq: PriorityQueue<i32> = PriorityQueue::new_lifo();
        pq.put(1);
        pq.put(2);
        pq.put(3);

        assert_eq!(pq.get(), Some(3));
        assert_eq!(pq.get(), Some(2));
        assert_eq!(pq.get(), Some(1));
    }

    #[test]
    fn lifo_reverse() {
        let mut pq: PriorityQueue<i32> = PriorityQueue::new_lifo();
        pq.put(1);
        pq.put(2);
        pq.put(3);
        pq.reverse();

        assert_eq!(pq.get(), Some(1));
        assert_eq!(pq.get(), Some(2));
        assert_eq!(pq.get(), Some(3));
    }

    #[test]
    fn peek() {
        let mut pq = PriorityQueue::new(|a: &i32, b: &i32| a.cmp(b));
        pq.put(5);
        pq.put(2);
        assert_eq!(pq.peek(), Some(&2));
        assert_eq!(pq.len(), 2); // peek doesn't remove
    }

    #[test]
    fn stability() {
        // Items with equal priority should come out in insertion order
        let mut pq = PriorityQueue::new(|a: &(i32, &str), b: &(i32, &str)| a.0.cmp(&b.0));
        pq.put((1, "first"));
        pq.put((1, "second"));
        pq.put((1, "third"));

        assert_eq!(pq.get(), Some((1, "first")));
        assert_eq!(pq.get(), Some((1, "second")));
        assert_eq!(pq.get(), Some((1, "third")));
    }

    #[test]
    fn empty_queue() {
        let mut pq = PriorityQueue::new(|a: &i32, b: &i32| a.cmp(b));
        assert!(pq.is_empty());
        assert_eq!(pq.get(), None);
        assert_eq!(pq.peek(), None);
    }
}
