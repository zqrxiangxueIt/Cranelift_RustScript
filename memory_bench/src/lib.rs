pub struct Node {
    pub value: i32,
    pub next: Option<Box<Node>>,
}

impl Node {
    pub fn new(value: i32) -> Self {
        Node { value, next: None }
    }
}

pub fn create_linked_list(size: usize) -> Option<Box<Node>> {
    let mut head = None;
    for i in 0..size {
        let mut node = Box::new(Node::new(i as i32));
        node.next = head;
        head = Some(node);
    }
    head
}

pub fn create_vec_allocation(size: usize) -> Vec<i32> {
    let mut vec = Vec::with_capacity(size);
    for i in 0..size {
        vec.push(i as i32);
    }
    vec
}
