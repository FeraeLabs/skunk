use graphviz_rust::dot_structures as dot;
use petgraph::{
    graph::NodeIndex,
    visit::IntoNodeReferences,
};

use super::{
    Graph,
    Node,
};

fn node_id(node_index: NodeIndex) -> dot::NodeId {
    dot::NodeId(dot::Id::Anonymous(format!("n{}", node_index.index())), None)
}

fn node_label(node: &Node) -> dot::Id {
    dot::Id::Html(match node.kind {
        super::NodeKind::Literal(value) => format!("{value}"),
        super::NodeKind::Variable => node.label.as_ref().map_or("?", AsRef::as_ref).to_owned(),
        super::NodeKind::Not => "~".to_owned(),
        super::NodeKind::And => "^".to_owned(),
        super::NodeKind::Or => "v".to_owned(),
    })
}

impl Graph {
    pub fn into_dot_graph(&self, graph_id: dot::Id) -> dot::Graph {
        let mut stmts = vec![];

        for (node_index, node) in self.graph.node_references() {
            stmts.push(dot::Stmt::Node(dot::Node {
                id: node_id(node_index),
                attributes: vec![dot::Attribute(
                    dot::Id::Plain("label".to_owned()),
                    node_label(node),
                )],
            }));
        }

        for edge_index in self.graph.edge_indices() {
            let edge = self.graph.edge_endpoints(edge_index).unwrap();
            stmts.push(dot::Stmt::Edge(dot::Edge {
                ty: dot::EdgeTy::Pair(
                    dot::Vertex::N(node_id(edge.0)),
                    dot::Vertex::N(node_id(edge.1)),
                ),
                attributes: vec![],
            }));
        }

        dot::Graph::DiGraph {
            id: graph_id,
            strict: true,
            stmts,
        }
    }
}
