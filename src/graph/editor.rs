use super::*;

use eframe::{egui, egui_glow, glow};
use egui::epaint::{CubicBezierShape, RectShape};
use egui::*;
#[cfg(feature = "persistence")]
use serde::{Deserialize, Serialize};
use slotmap::SecondaryMap;

#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphEditor {
    pub graph: Graph<NodeData, DataType, ValueType>,
    pub node_order: Vec<NodeId>,
    pub connection_in_progress: Option<(NodeId, SlotId)>,
    pub selected_nodes: Vec<NodeId>,
    pub ongoing_box_selection: Option<egui::Pos2>,
    pub node_positions: SecondaryMap<NodeId, egui::Pos2>,
    pub node_finder: Option<NodeFinder<NodeTemplate>>,
}

impl Default for GraphEditor {
    fn default() -> Self {
        Self {
            graph: Default::default(),
            node_order: Default::default(),
            connection_in_progress: Default::default(),
            selected_nodes: Default::default(),
            ongoing_box_selection: Default::default(),
            node_positions: Default::default(),
            node_finder: Default::default(),
            //pan_zoom: Default::default(),
            //_user_state: Default::default(),
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeFinder<NodeTemplate> {
    pub query: String,
    /// Reset every frame. When set, the node finder will be moved at that position
    pub position: Option<egui::Pos2>,
    pub just_spawned: bool,
    //_phantom: PhantomData<NodeTemplate>,
}

/// Nodes communicate certain events to the parent graph when drawn. There is
/// one special `User` variant which can be used by users as the return value
/// when executing some custom actions in the UI of the node.
#[derive(Clone, Debug)]
pub enum NodeResponse {
    ConnectEventStarted(NodeId, SlotId),
    ConnectEventEnded {
        output: SlotId,
        input: SlotId,
    },
    CreatedNode(NodeId),
    SelectNode(NodeId),
    /// As a user of this library, prefer listening for `DeleteNodeFull` which
    /// will also contain the user data for the deleted node.
    DeleteNodeUi(NodeId),
    /// Emitted when a node is deleted. The node will no longer exist in the
    /// graph after this response is returned from the draw function, but its
    /// contents are passed along with the event.
    DeleteNodeFull {
        node_id: NodeId,
        //node: Node<NodeData>,
    },
    DisconnectEvent {
        output: SlotId,
        input: SlotId,
    },
    /// Emitted when a node is interacted with, and should be raised
    RaiseNode(NodeId),
    MoveNode {
        node: NodeId,
        drag_delta: Vec2,
    },
}

pub struct NodeWidget<'a> {
    pub position: egui::Pos2,
    pub graph: &'a mut Graph<NodeData, DataType, ValueType>,
    pub port_locations: std::collections::HashMap<AnyParameterId, egui::Pos2>,
    pub node_rects: std::collections::HashMap<NodeId, egui::Rect>,
    pub node_id: NodeId,
    pub ongoing_drag: Option<(NodeId, SlotId)>,
    pub selected: bool,
    pub pan: egui::Vec2,
}

impl<'a> NodeWidget<'a> {
    pub const MAX_NODE_SIZE: [f32; 2] = [200.0, 200.0];

    /// Draws this node. Also fills in the list of port locations with all of its ports.
    /// Returns responses indicating multiple events.
    fn show(
        self,
        ui: &mut egui::Ui,
        user_state: &mut UserState,
    ) -> Vec<NodeResponse<UserResponse, NodeData>> {
        use egui::*;

        let mut ui = ui.child_ui_with_id_source(
            Rect::from_min_size(*self.position + self.pan, Self::MAX_NODE_SIZE.into()),
            Layout::default(),
            self.node_id,
        );

        let margin = egui::vec2(15.0, 5.0);
        let mut responses = Vec::<NodeResponse<UserResponse, NodeData>>::new();

        let background_color = ui.visuals().widgets.inactive.bg_fill;
        let text_color = ui.visuals().widgets.inactive.text_color();

        ui.visuals_mut().widgets.noninteractive.fg_stroke = Stroke::new(2.0, text_color);

        // Preallocate shapes to paint below contents
        let outline_shape = ui.painter().add(Shape::Noop);
        let background_shape = ui.painter().add(Shape::Noop);

        let outer_rect_bounds = ui.available_rect_before_wrap();

        let mut inner_rect = outer_rect_bounds.shrink2(margin);

        // Make sure we don't shrink to the negative:
        inner_rect.max.x = inner_rect.max.x.max(inner_rect.min.x);
        inner_rect.max.y = inner_rect.max.y.max(inner_rect.min.y);

        let mut child_ui = ui.child_ui(inner_rect, *ui.layout());

        // Get interaction rect from memory, it may expand after the window response on resize.
        let interaction_rect = ui
            .ctx()
            .memory_mut(|mem| {
                mem.data
                    .get_temp::<Rect>(child_ui.id())
                    .map(|stored| stored.0)
            })
            .unwrap_or(outer_rect_bounds);
        // After 0.20, layers added over others can block hover interaction. Call this first
        // before creating the node content.
        let window_response = ui.interact(
            interaction_rect,
            Id::new((self.node_id, "window")),
            Sense::click_and_drag(),
        );

        let mut title_height = 0.0;

        let mut input_port_heights = vec![];
        let mut output_port_heights = vec![];

        child_ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add(Label::new(
                    RichText::new(&self.graph[self.node_id].label)
                        .text_style(TextStyle::Button)
                        .color(text_color),
                ));
                responses.extend(
                    self.graph[self.node_id]
                        .user_data
                        .top_bar_ui(ui, self.node_id, self.graph, user_state)
                        .into_iter(),
                );
                ui.add_space(8.0); // The size of the little cross icon
            });
            ui.add_space(margin.y);
            title_height = ui.min_size().y;

            // First pass: Draw the inner fields. Compute port heights
            let inputs = self.graph[self.node_id].inputs.clone();
            for (param_name, param_id) in inputs {
                if self.graph[param_id].shown_inline {
                    let height_before = ui.min_rect().bottom();
                    // NOTE: We want to pass the `user_data` to
                    // `value_widget`, but we can't since that would require
                    // borrowing the graph twice. Here, we make the
                    // assumption that the value is cheaply replaced, and
                    // use `std::mem::take` to temporarily replace it with a
                    // dummy value. This requires `ValueType` to implement
                    // Default, but results in a totally safe alternative.
                    let mut value = std::mem::take(&mut self.graph[param_id].value);

                    if self.graph.connection(param_id).is_some() {
                        let node_responses = value.value_widget_connected(
                            &param_name,
                            self.node_id,
                            ui,
                            user_state,
                            &self.graph[self.node_id].user_data,
                        );

                        responses.extend(node_responses.into_iter().map(NodeResponse::User));
                    } else {
                        let node_responses = value.value_widget(
                            &param_name,
                            self.node_id,
                            ui,
                            user_state,
                            &self.graph[self.node_id].user_data,
                        );

                        responses.extend(node_responses.into_iter().map(NodeResponse::User));
                    }

                    self.graph[self.node_id].user_data.separator(
                        ui,
                        self.node_id,
                        param_id,
                        self.graph,
                        user_state,
                    );

                    self.graph[param_id].value = value;

                    let height_after = ui.min_rect().bottom();
                    input_port_heights.push((height_before + height_after) / 2.0);
                }
            }

            let outputs = self.graph[self.node_id].outputs.clone();
            for (param_name, param_id) in outputs {
                let height_before = ui.min_rect().bottom();
                responses.extend(
                    self.graph[self.node_id]
                        .user_data
                        .output_ui(ui, self.node_id, self.graph, user_state, &param_name)
                        .into_iter(),
                );

                self.graph[self.node_id].user_data.separator(
                    ui,
                    self.node_id,
                    param_id,
                    self.graph,
                    user_state,
                );

                let height_after = ui.min_rect().bottom();
                output_port_heights.push((height_before + height_after) / 2.0);
            }

            responses.extend(
                self.graph[self.node_id]
                    .user_data
                    .bottom_ui(ui, self.node_id, self.graph, user_state)
                    .into_iter(),
            );
        });

        // Second pass, iterate again to draw the ports. This happens outside
        // the child_ui because we want ports to overflow the node background.

        let outer_rect = child_ui.min_rect().expand2(margin);
        let port_left = outer_rect.left();
        let port_right = outer_rect.right();

        // Save expanded rect to memory.
        ui.ctx()
            .memory_mut(|mem| mem.data.insert_temp(child_ui.id(), outer_rect));

        // Input ports
        for ((_, param), port_height) in self.graph[self.node_id]
            .inputs
            .iter()
            .zip(input_port_heights.into_iter())
        {
            let should_draw = match self.graph[*param].kind() {
                InputParamKind::ConnectionOnly => true,
                InputParamKind::ConstantOnly => false,
                InputParamKind::ConnectionOrConstant => true,
            };

            if should_draw {
                let pos_left = pos2(port_left, port_height);
                self.draw_port(
                    ui,
                    self.graph,
                    self.node_id,
                    user_state,
                    pos_left,
                    &mut responses,
                    *param,
                    self.port_locations,
                    self.ongoing_drag,
                    self.graph.connection(*param).is_some(),
                );
            }
        }

        // Output ports
        for ((_, param), port_height) in self.graph[self.node_id]
            .outputs
            .iter()
            .zip(output_port_heights.into_iter())
        {
            let pos_right = pos2(port_right, port_height);
            self.draw_port(
                ui,
                self.graph,
                self.node_id,
                user_state,
                pos_right,
                &mut responses,
                *param,
                self.port_locations,
                self.ongoing_drag,
                false,
            );
        }

        // Draw the background shape.
        // NOTE: This code is a bit more involved than it needs to be because egui
        // does not support drawing rectangles with asymmetrical round corners.

        let (shape, outline) = {
            let rounding_radius = 4.0;
            let rounding = Rounding::same(rounding_radius);

            let titlebar_height = title_height + margin.y;
            let titlebar_rect =
                Rect::from_min_size(outer_rect.min, vec2(outer_rect.width(), titlebar_height));
            let titlebar = Shape::Rect(RectShape {
                rect: titlebar_rect,
                rounding,
                fill: self.graph[self.node_id]
                    .user_data
                    .titlebar_color(ui, self.node_id, self.graph, user_state)
                    .unwrap_or_else(|| background_color.lighten(0.8)),
                stroke: Stroke::NONE,
                fill_texture_id: Default::default(),
                uv: Rect::ZERO,
            });

            let body_rect = Rect::from_min_size(
                outer_rect.min + vec2(0.0, titlebar_height - rounding_radius),
                vec2(outer_rect.width(), outer_rect.height() - titlebar_height),
            );
            let body = Shape::Rect(RectShape {
                rect: body_rect,
                rounding: Rounding::none(),
                fill: background_color,
                stroke: Stroke::NONE,
                fill_texture_id: Default::default(),
                uv: Rect::ZERO,
            });

            let bottom_body_rect = Rect::from_min_size(
                body_rect.min + vec2(0.0, body_rect.height() - titlebar_height * 0.5),
                vec2(outer_rect.width(), titlebar_height),
            );
            let bottom_body = Shape::Rect(RectShape {
                rect: bottom_body_rect,
                rounding,
                fill: background_color,
                stroke: Stroke::NONE,
                fill_texture_id: Default::default(),
                uv: Rect::ZERO,
            });

            let node_rect = titlebar_rect.union(body_rect).union(bottom_body_rect);
            let outline = if self.selected {
                Shape::Rect(RectShape {
                    rect: node_rect.expand(1.0),
                    rounding,
                    fill: Color32::WHITE.lighten(0.8),
                    stroke: Stroke::NONE,
                    fill_texture_id: Default::default(),
                    uv: Rect::ZERO,
                })
            } else {
                Shape::Noop
            };

            // Take note of the node rect, so the editor can use it later to compute intersections.
            self.node_rects.insert(self.node_id, node_rect);

            (Shape::Vec(vec![titlebar, body, bottom_body]), outline)
        };

        ui.painter().set(background_shape, shape);
        ui.painter().set(outline_shape, outline);

        // --- Interaction ---

        // Titlebar buttons
        let can_delete = self.graph.nodes[self.node_id].user_data.can_delete(
            self.node_id,
            self.graph,
            user_state,
        );

        if can_delete && Self::close_button(ui, outer_rect).clicked() {
            responses.push(NodeResponse::DeleteNodeUi(self.node_id));
        };

        // Movement
        let drag_delta = window_response.drag_delta();
        if drag_delta.length_sq() > 0.0 {
            responses.push(NodeResponse::MoveNode {
                node: self.node_id,
                drag_delta,
            });
            responses.push(NodeResponse::RaiseNode(self.node_id));
        }

        // Node selection
        //
        // HACK: Only set the select response when no other response is active.
        // This prevents some issues.
        if responses.is_empty() && window_response.clicked_by(PointerButton::Primary) {
            responses.push(NodeResponse::SelectNode(self.node_id));
            responses.push(NodeResponse::RaiseNode(self.node_id));
        }

        responses
    }

    fn close_button(ui: &mut Ui, node_rect: Rect) -> Response {
        // Measurements
        let margin = 8.0;
        let size = 10.0;
        let stroke_width = 2.0;
        let offs = margin + size / 2.0;

        let position = pos2(node_rect.right() - offs, node_rect.top() + offs);
        let rect = Rect::from_center_size(position, vec2(size, size));
        let resp = ui.allocate_rect(rect, Sense::click());

        let color = ui.visuals().widgets.active.bg_fill;

        let stroke = Stroke {
            width: stroke_width,
            color,
        };

        ui.painter()
            .line_segment([rect.left_top(), rect.right_bottom()], stroke);
        ui.painter()
            .line_segment([rect.right_top(), rect.left_bottom()], stroke);

        resp
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_port<NodeData, DataType, ValueType, UserResponse, UserState>(
        ui: &mut egui::Ui,
        graph: &Graph<NodeData, DataType, ValueType>,
        node_id: NodeId,
        user_state: &mut UserState,
        port_pos: egui::Pos2,
        responses: &mut Vec<NodeResponse<UserResponse, NodeData>>,
        param_id: SlotId,
        port_locations: &mut std::collections::HashMap<SlotId, Pos2>,
        ongoing_drag: Option<(NodeId, SlotId)>,
        is_connected_input: bool,
    ) where
        DataType: DataTypeTrait<UserState>,
        UserResponse: UserResponseTrait,
        NodeData: NodeDataTrait,
    {
        use egui::*;

        let port_type = graph.any_param_type(param_id).unwrap();

        let port_rect = Rect::from_center_size(port_pos, egui::vec2(10.0, 10.0));

        let sense = if ongoing_drag.is_some() {
            Sense::hover()
        } else {
            Sense::click_and_drag()
        };

        let resp = ui.allocate_rect(port_rect, sense);

        // Check if the distance between the port and the mouse is the distance to connect
        let close_enough = if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            port_rect.center().distance(pointer_pos) < DISTANCE_TO_CONNECT
        } else {
            false
        };

        let port_color = if close_enough {
            Color32::WHITE
        } else {
            port_type.data_type_color(user_state)
        };
        ui.painter()
            .circle(port_rect.center(), 5.0, port_color, Stroke::NONE);

        if resp.drag_started() {
            if is_connected_input {
                let input = param_id.assume_input();
                let corresp_output = graph
                    .connection(input)
                    .expect("Connection data should be valid");
                responses.push(NodeResponse::DisconnectEvent {
                    input: param_id.assume_input(),
                    output: corresp_output,
                });
            } else {
                responses.push(NodeResponse::ConnectEventStarted(node_id, param_id));
            }
        }

        if let Some((origin_node, origin_param)) = ongoing_drag {
            if origin_node != node_id {
                // Don't allow self-loops
                if graph.any_param_type(origin_param).unwrap() == port_type
                    && close_enough
                    && ui.input(|i| i.pointer.any_released())
                {
                    match (param_id, origin_param) {
                        (input, output) | (output, input) => {
                            responses.push(NodeResponse::ConnectEventEnded { input, output });
                        }
                        _ => { /* Ignore in-in or out-out connections */ }
                    }
                }
            }
        }

        port_locations.insert(param_id, port_rect.center());
    }
}
