use crate::stream::Streamer;
use ark_ff::Field;
use ark_std::borrow::Borrow;

#[derive(Clone, Copy)]
pub struct LookupSortedStreamer<'a, F, S, SA> {
    base_streamer: &'a S,
    addr_streamer: &'a SA,
    y: F,
    z: F,
}

impl<'a, F, S, SA> LookupSortedStreamer<'a, F, S, SA> {
    pub fn new(base_streamer: &'a S, addr_streamer: &'a SA, y: F, z: F) -> Self {
        Self {
            base_streamer,
            addr_streamer,
            y,
            z,
        }
    }
}

impl<'a, F, S, SA> Streamer for LookupSortedStreamer<'a, F, S, SA>
where
    F: Field,
    S: Streamer,
    SA: Streamer,
    S::Item: Borrow<F> + Clone,
    SA::Item: Borrow<usize>,
{
    type Item = F;

    type Iter = AlgHashIterator<F, SortedIterator<S::Item, S::Iter, SA::Iter>>;

    fn stream(&self) -> Self::Iter {
        let base_iter = self.base_streamer.stream();
        let addr_iter = self.addr_streamer.stream();
        AlgHashIterator::new(
            SortedIterator::new(base_iter, addr_iter, self.base_streamer.len()),
            self.y,
            self.z,
        )
    }

    fn len(&self) -> usize {
        self.base_streamer.len() + self.addr_streamer.len()
    }
}

pub struct SortedIterator<T, I, J>
where
    I: Iterator<Item = T>,
    J: Iterator,
    J::Item: Borrow<usize>,
{
    current_it: usize,
    cache: Option<T>,
    it: I,
    current_address: Option<J::Item>,
    addresses: J,
}

impl<T, I, J> SortedIterator<T, I, J>
where
    I: Iterator<Item = T>,
    J: Iterator,
    J::Item: Borrow<usize>,
{
    fn new(it: I, mut addresses: J, len: usize) -> Self {
        let current_it = len;
        let cache = None;
        let current_address = addresses.next();
        Self {
            current_it,
            cache,
            it,
            current_address,
            addresses,
        }
    }
}

impl<T, I, J> Iterator for SortedIterator<T, I, J>
where
    T: Clone,
    I: Iterator<Item = T>,
    J: Iterator,
    J::Item: Borrow<usize>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // if we have an element from the previous iteration, return it.
        match &self.current_address {
            None => self.it.next(),
            Some(current_address) => {
                let current_address = *current_address.borrow();
                if self.current_it > current_address {
                    self.current_it -= 1;
                    self.cache = self.it.next();
                    self.cache.clone()
                } else if self.current_it == current_address {
                    self.current_address = self.addresses.next();
                    self.cache.clone()
                } else {
                    // self.current_it < self.current_address
                    panic!("address index is not decreasing. Perhaps wrong sorting?")
                }
            }
        }
    }
}

pub struct AlgHashIterator<F, I>
where
    I: Iterator,
{
    y1z: F,
    z: F,
    first: F,
    previous: Option<F>,
    it: I,
}

impl<F, I> AlgHashIterator<F, I>
where
    F: Field,
    I: Iterator,
    I::Item: Borrow<F> + Clone,
{
    fn new(mut it: I, y: F, z: F) -> Self {
        let next = *it.next().unwrap().borrow();
        Self {
            z,
            y1z: y * (F::one() + z),
            it,
            first: next,
            previous: Some(next),
        }
    }
}

impl<F, I> Iterator for AlgHashIterator<F, I>
where
    F: Field,
    I: Iterator,
    I::Item: Borrow<F>,
{
    type Item = F;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.it.next(), self.previous) {
            (Some(current), Some(previous)) => {
                let current = *current.borrow();
                self.previous = Some(current);
                Some(self.y1z + previous.borrow() + self.z * current)
            }
            (None, Some(previous)) => {
                self.previous = None;
                Some(self.y1z + previous.borrow() + self.z * self.first)
            }
            (None, None) => None,
            (Some(_), None) => panic!(
                "Something wrong with the iterator: previous position is None, current is Some(_)."
            ),
        }
    }
}

#[test]
fn test_sorted_iterator() {
    let base = vec!["1", "2", "3", "4"];
    let addresses = vec![2usize, 2, 2, 1, 0, 0, 0];
    let expected = vec!["4", "3", "3", "3", "3", "2", "2", "1", "1", "1", "1"];
    let sorted_iterator =
        SortedIterator::new(base.iter().rev(), addresses.iter().cloned(), base.len())
            .cloned()
            .collect::<Vec<_>>();
    assert_eq!(sorted_iterator, expected);

    let base = vec!["1", "2", "3", "4", "5", "6"];
    let addresses = vec![4, 3, 3, 2, 1, 1, 1, 1, 0, 0];
    let expected = vec![
        "6", "5", "5", "4", "4", "4", "3", "3", "2", "2", "2", "2", "2", "1", "1", "1",
    ];
    let sorted_iterator =
        SortedIterator::new(base.iter().rev(), addresses.iter().cloned(), base.len())
            .cloned()
            .collect::<Vec<_>>();
    assert_eq!(sorted_iterator, expected);
}

#[test]
fn test_sorted_stream() {
    use ark_bls12_381::Fr;
    use ark_ff::One;
    use ark_std::rand::Rng;
    use ark_std::UniformRand;

    let rng = &mut ark_std::test_rng();
    let set_size = 5;
    let subset_size = 10;
    let test_vector = (0..set_size).map(|_| Fr::rand(rng)).collect::<Vec<_>>();

    // assume the subset indices are sorted.
    let mut subset_indices = (0..subset_size)
        .map(|_| rng.gen_range(0..set_size))
        .collect::<Vec<_>>();
    subset_indices.sort();
    // create the array for merged indices and the sorted vector `w `
    let mut merged_indices = subset_indices.clone();
    merged_indices.extend(0..set_size);
    merged_indices.sort();
    merged_indices.reverse();
    let w = merged_indices
        .iter()
        .map(|&i| test_vector[i])
        .collect::<Vec<_>>();

    let y = Fr::rand(rng);
    let z = Fr::rand(rng);
    let len = set_size + subset_size;
    let ans = (0..len)
        .map(|i| y * (Fr::one() + z) + w[i] + z * w[(i + 1) % len])
        .collect::<Vec<_>>();

    let subset_indices_stream = subset_indices.iter().rev().cloned().collect::<Vec<_>>();
    let test_vector_stream = test_vector.iter().rev().cloned().collect::<Vec<_>>();
    let sorted_stream = LookupSortedStreamer::new(
        &test_vector_stream.as_slice(),
        &subset_indices_stream.as_slice(),
        y,
        z,
    )
    .stream()
    .collect::<Vec<_>>();
    assert_eq!(sorted_stream, ans);
}
