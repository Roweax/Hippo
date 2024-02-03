use slotmap::SecondaryMap;
use slotmap::SlotMap;

use super::node::*;

pub struct Graph<NodeData> {
    pub nodes: SlotMap<NodeId, Node<NodeData>>,
    // pub inputs: SlotMap<InputId, InputParam<DataType, ValueType>>,
    // pub outputs: SlotMap<OutputId, OutputParam<DataType>>,
    pub connections: SecondaryMap<InputId, OutputId>,
}

impl<NodeData> Graph<NodeData> {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::default(),
            connections: SecondaryMap::default(),
        }
    }

    pub fn set_root(&mut self, node: NodeId) {}

    pub fn add_node(&mut self, label: String, data: NodeData) -> NodeId {
        let node_id = self.nodes.insert_with_key(|node_id| {
            Node {
                id: node_id,
                label,
                // These get filled in later by the user function
                inputs: Vec::default(),
                outputs: Vec::default(),
                data,
            }
        });
        node_id
    }
}
