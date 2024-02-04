slotmap::new_key_type! { pub struct NodeId; }
slotmap::new_key_type! { pub struct SlotId; }

pub struct Node<T> {
    pub id: NodeId,
    pub label: String,
    pub inputs: Vec<(String, SlotId)>,
    pub outputs: Vec<(String, SlotId)>,
    pub data: T,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "persistence", derive(Serialize, Deserialize))]
pub struct Input<DataType, ValueType> {
    pub id: SlotId,
    pub typ: DataType,
    pub value: ValueType,
    pub node: NodeId,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "persistence", derive(Serialize, Deserialize))]
pub struct Output<DataType> {
    pub id: SlotId,
    pub node: NodeId,
    pub typ: DataType,
}
