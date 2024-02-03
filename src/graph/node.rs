slotmap::new_key_type! { pub struct NodeId; }
slotmap::new_key_type! { pub struct InputId; }
slotmap::new_key_type! { pub struct OutputId; }

pub struct Node<T> {
    pub id: NodeId,
    pub label: String,
    pub inputs: Vec<(String, InputId)>,
    pub outputs: Vec<(String, OutputId)>,
    pub data: T,
}
