use ark_std::borrow::Borrow;

use ark_ff::PrimeField;

use crate::misc::{MatrixElement, PartialTensor, TENSOR_EXPANSION, TENSOR_EXPANSION_LOG};
use crate::stream::Streamer;

#[derive(Clone, Copy)]
pub(crate) struct MatrixTensor<'a, F, SC>
where
    F: PrimeField,
    SC: Streamer,
    SC::Item: Borrow<MatrixElement<F>>,
{
    matrix: SC,
    tensor: &'a PartialTensor<F>,
    len: usize,
}

impl<'a, F, SC> MatrixTensor<'a, F, SC>
where
    F: PrimeField,
    SC: Streamer,
    SC::Item: Borrow<MatrixElement<F>>,
{
    pub fn new(matrix: SC, tensor: &'a PartialTensor<F>, len: usize) -> Self {
        MatrixTensor {
            matrix,
            tensor,
            len,
        }
    }
}

impl<'a, F, SC> Streamer for MatrixTensor<'a, F, SC>
where
    F: PrimeField,
    SC: Streamer,
    SC::Item: Borrow<MatrixElement<F>>,
{
    type Item = F;
    type Iter = MatrixTensorIter<'a, F, SC::Iter>;

    fn stream(&self) -> Self::Iter {
        MatrixTensorIter {
            it: self.matrix.stream(),
            tensor: self.tensor,
        }
    }

    fn len(&self) -> usize {
        self.len
    }
}

pub(crate) struct MatrixTensorIter<'a, F, I>
where
    F: PrimeField,
    I: Iterator,
    I::Item: Borrow<MatrixElement<F>>,
{
    it: I,
    tensor: &'a PartialTensor<F>,
}

impl<'a, F, I> Iterator for MatrixTensorIter<'a, F, I>
where
    F: PrimeField,
    I: Iterator,
    I::Item: Borrow<MatrixElement<F>>,
{
    type Item = F;

    fn next(&mut self) -> Option<Self::Item> {
        let mut result = F::zero();

        for e in &mut self.it {
            match *e.borrow() {
                MatrixElement::Element((mut value, index)) if !value.is_zero() => {
                    for (i, r) in self.tensor.iter().enumerate() {
                        let selection_index = (index >> (i * TENSOR_EXPANSION_LOG)) & TENSOR_EXPANSION;
                        if selection_index != 0 {
                            value *= r[selection_index - 1];
                        }
                    }
                    result += value;
                }
                MatrixElement::EOL => {
                    return Some(result);
                }
                _ => (),
            }
        }
        None
    }
}

#[test]
fn test_matrix_tensor_stream() {
    use crate::misc::expand_tensor;
    use crate::stream::dummy::DiagonalMatrixStreamer;
    use ark_bls12_381::Fr as F;
    use ark_ff::One;
    use ark_std::test_rng;
    use ark_std::UniformRand;

    let rng = &mut test_rng();
    let r = F::rand(rng);
    let n = 4;
    let one_tensor = [F::one(), F::one()];
    let expanded_one_tensor = expand_tensor(&one_tensor);

    let matrix = DiagonalMatrixStreamer::new(r, n);
    let matrix_tensor = MatrixTensor::new(matrix, &expanded_one_tensor, n);
    let mut stream = matrix_tensor.stream();
    assert_eq!(stream.next(), Some(r));
    assert_eq!(stream.next(), Some(r));
    assert_eq!(stream.next(), Some(r));
    assert_eq!(stream.next(), Some(r));
    assert!(stream.next().is_none());

    let random_tensor = [F::rand(rng), F::rand(rng)];
    let expanded_random_tensor = expand_tensor(&random_tensor);
    let matrix_tensor = MatrixTensor::new(matrix, &expanded_random_tensor, n);
    let mut stream = matrix_tensor.stream();
    assert_eq!(stream.next(), Some(r * random_tensor[0] * random_tensor[1]));
    assert_eq!(stream.next(), Some(r * random_tensor[1]));
    assert_eq!(stream.next(), Some(r * random_tensor[0]));
    assert_eq!(stream.next(), Some(r));
}

#[test]
fn test_matrix_tensor_len() {
    use crate::stream::dummy::DiagonalMatrixStreamer;
    use ark_bls12_381::Fr as F;
    use ark_ff::One;
    use ark_std::test_rng;
    use ark_std::UniformRand;

    use crate::circuit::generate_relation;
    use crate::circuit::random_circuit;
    use crate::misc::expand_tensor;
    use crate::misc::matrix_into_row_major_slice;

    let rng = &mut test_rng();
    let r = F::rand(rng);
    let n = 4;
    let one_tensor = [F::one(), F::one()];

    let matrix = DiagonalMatrixStreamer::new(r, n);
    let expanded_one_tensor = expand_tensor(&one_tensor);
    let matrix_tensor = MatrixTensor::new(matrix, &expanded_one_tensor, n);

    assert_eq!(matrix_tensor.len(), matrix_tensor.stream().count());

    let n = 1 << 10;
    let circuit = random_circuit(rng, n, n);
    let r1cs = generate_relation(circuit);
    let matrix = matrix_into_row_major_slice(&r1cs.a, n);
    let matrix_tensor = MatrixTensor::new(matrix.as_slice(), &expanded_one_tensor, n);
    assert_eq!(matrix_tensor.len(), matrix_tensor.stream().count());
}

#[test]
fn test_matrix_tensor() {
    use crate::misc::expand_tensor;
    use ark_bls12_381::Fr;
    use ark_ff::{One, Zero};
    use ark_std::test_rng;
    use ark_std::UniformRand;

    use crate::stream::dummy::DiagonalMatrixStreamer;
    use MatrixElement::*;

    type F = Fr;
    let one = F::one();
    let rng = &mut test_rng();

    let matrix = vec![
        Element((one, 0)),
        Element((one, 1)),
        Element((one, 2)),
        Element((one, 3)),
        EOL,
        Element((one, 1)),
        EOL,
        Element((one, 2)),
        EOL,
        EOL,
    ];
    let r = F::rand(rng);
    let tensor = vec![r, r * r];
    let expanded_tensor = expand_tensor(&tensor);
    let mt = MatrixTensor::new(matrix.as_slice(), &expanded_tensor, 4);
    let mut product = mt.stream();
    let expected = r * r * r + r * r + r + one;
    assert_eq!(product.next(), Some(expected));
    assert_eq!(product.next(), Some(r));
    assert_eq!(product.next(), Some(r * r));
    assert_eq!(product.next(), Some(F::zero()));
    assert_eq!(product.next(), None);

    let challenges = [F::rand(rng), F::rand(rng), F::rand(rng), F::rand(rng)];
    let identity = DiagonalMatrixStreamer::new(F::one(), 1 << 4);

    let expanded_challenges = expand_tensor(&challenges);
    let mt = MatrixTensor::new(identity, &expanded_challenges, 1 << 4);
    let mut got = mt.stream().collect::<Vec<_>>();
    got.reverse();
    let expected = crate::misc::tensor(&challenges);
    assert_eq!(got, expected);
}
