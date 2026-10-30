//! A task queue ordered like gcsim's `task.Handler`: by frame due, then by
//! the order tasks were added. Tasks are plain values, so queuing one
//! allocates nothing once the heap has grown.

pub struct Queue<T: Copy> {
    heap: Vec<(i32, u32, T)>,
    counter: u32,
}

impl<T: Copy> Queue<T> {
    pub fn new() -> Self {
        Queue { heap: Vec::with_capacity(64), counter: 0 }
    }

    pub fn clear(&mut self) {
        self.heap.clear();
        self.counter = 0;
    }

    #[inline]
    fn less(a: &(i32, u32, T), b: &(i32, u32, T)) -> bool {
        (a.0, a.1) < (b.0, b.1)
    }

    pub fn add(&mut self, f: i32, t: T, delay: i32) {
        let item = (f + delay, self.counter, t);
        self.counter += 1;
        self.heap.push(item);
        let mut j = self.heap.len() - 1;
        while j > 0 {
            let i = (j - 1) / 2;
            if !Self::less(&item, &self.heap[i]) {
                break;
            }
            self.heap[j] = self.heap[i];
            j = i;
        }
        self.heap[j] = item;
    }

    /// Removes and returns the earliest task due by frame f.
    #[inline]
    pub fn pop_due(&mut self, f: i32) -> Option<T> {
        match self.heap.first() {
            Some(top) if top.0 <= f => {}
            _ => return None,
        }
        let n = self.heap.len() - 1;
        let top = self.heap[0];
        let last = self.heap[n];
        self.heap.truncate(n);
        if n > 0 {
            let mut i = 0;
            loop {
                let mut j = 2 * i + 1;
                if j >= n {
                    break;
                }
                if j + 1 < n && Self::less(&self.heap[j + 1], &self.heap[j]) {
                    j += 1;
                }
                if !Self::less(&self.heap[j], &last) {
                    break;
                }
                self.heap[i] = self.heap[j];
                i = j;
            }
            self.heap[i] = last;
        }
        Some(top.2)
    }

    /// Delays every queued task, as hitlag does.
    pub fn extend(&mut self, delay: i32) {
        for t in &mut self.heap {
            t.0 += delay;
        }
    }
}
