use ndarray::prelude::*;

use super::super::graph;

#[test]
fn create_graph() {
    let g = graph::Graph::<f32>::new();
}

#[test]
fn create_array() {
    let a = array![[1., 2., 3.], [4., 5., 6.],];
    assert_eq!(a.ndim(), 2);
    assert_eq!(a.map(|x| *x + 1.), arr2(&[[2., 3., 4.], [5., 6., 7.]]));
}
