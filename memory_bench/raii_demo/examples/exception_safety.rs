use raii_demo::DynamicArray;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Tracked(usize, Arc<AtomicUsize>);

impl Drop for Tracked {
    fn drop(&mut self) {
        println!("Dropping element {}", self.0);
        self.1.fetch_add(1, Ordering::SeqCst);
    }
}

fn main() {
    println!("--- Exception Safety & RAII Example ---");
    let drop_count = Arc::new(AtomicUsize::new(0));
    
    {
        let mut arr = DynamicArray::new();
        for i in 0..3 {
            arr.push(Tracked(i, drop_count.clone()));
        }
        println!("Array created with 3 elements.");
        // Scope ends here, arr is dropped
    }

    println!("Total elements dropped: {}", drop_count.load(Ordering::SeqCst));
    assert_eq!(drop_count.load(Ordering::SeqCst), 3);
}
