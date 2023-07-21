use parity_scale_codec::Encode;

pub const MSG_BYTES_LIMIT: usize = 10;

struct Limiter<'a, D: Encode> {
    msg: &'a [D],
    low: usize,
}

impl<'a, D: Encode> Limiter<'a, D> {
    fn new(msg: &'a [D]) -> Self {
        Self { msg, low: 0 }
    }

    fn next_largest_msg(&mut self) -> Option<&'a [D]> {
        if self.low == self.msg.len() {
            return None;
        }
        let idx = self.bs_by_encode()?;
        let old_low = self.low;
        self.low = idx;
        Some(&self.msg[old_low..idx])
    }

    fn bs_by_encode(&self) -> Option<usize> {
        let mut left = self.low;
        let mut right = self.msg.len();

        // we are looking
        let upper_bound = MSG_BYTES_LIMIT + 1;

        while left < right {
            let mid = (left + right) / 2;

            let encoded_size = self.msg[self.low..mid].encoded_size();

            if encoded_size <= upper_bound {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == self.low {
            return None;
        }
        let encoded_size = self.msg[self.low..left].encoded_size();

        if encoded_size <= MSG_BYTES_LIMIT {
            Some(left)
        } else {
            Some(left - 1)
        }
    }
}

impl<'a, D: Encode> Iterator for Limiter<'a, D> {
    type Item = &'a [D];

    fn next(&mut self) -> Option<Self::Item> {
        self.next_largest_msg()
    }
}

#[cfg(test)]
mod tests {
    use parity_scale_codec::Encode;

    use crate::sync::message_limiter::Limiter;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct EncodeToSize(usize);

    impl Encode for EncodeToSize {
        fn size_hint(&self) -> usize {
            self.0
        }
        fn encode(&self) -> Vec<u8> {
            vec![0; self.0]
        }
    }

    fn sized(size: usize) -> EncodeToSize {
        EncodeToSize(size)
    }

    #[test]
    fn xxx() {
        let v = vec![sized(5), sized(6), sized(7)];

        let mut lim = Limiter::new(&v);

        assert_eq!(Some(&v[..1]), lim.next_largest_msg())
    }
    #[test]
    fn xxx1() {
        let v = vec![sized(1), sized(2), sized(3)];

        let mut lim = Limiter::new(&v);

        assert_eq!(Some(&v[..]), lim.next_largest_msg())
    }
    #[test]
    fn xxx2() {
        let v = vec![sized(1), sized(2), sized(7)];

        let mut lim = Limiter::new(&v);

        assert_eq!(Some(&v[..2]), lim.next_largest_msg())
    }
    #[test]
    fn xxx3() {
        let v = vec![];

        let mut lim = Limiter::<EncodeToSize>::new(&v);

        assert_eq!(None, lim.next_largest_msg())
    }
    #[test]
    fn xxx4() {
        let v = vec![sized(10)];

        let mut lim = Limiter::new(&v);

        assert_eq!(None, lim.next_largest_msg())
    }

    #[test]
    fn xxx5() {
        let v = vec![sized(5), sized(6), sized(7)];

        let mut lim = Limiter::new(&v);

        assert_eq!(Some(&v[..1]), lim.next());
        assert_eq!(Some(&v[1..2]), lim.next());
        assert_eq!(Some(&v[2..3]), lim.next());
        assert_eq!(None, lim.next());
    }
}
