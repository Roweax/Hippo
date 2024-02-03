use ndarray::prelude::*;

enum Value {
    Int32(i32),
    Float32(f32),
    Matrix(Array2<f32>),
}
