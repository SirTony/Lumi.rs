use std::vec::IntoIter;
use std::iter::IntoIterator;
use std::collections::VecDeque;

#[derive( Clone )]
pub struct BufferedPeekable<T> {
    iter: IntoIter<T>,
    buf: VecDeque<T>,
}

impl<T> BufferedPeekable<T> {
    pub fn new( v: Vec<T> ) -> BufferedPeekable<T> {
        BufferedPeekable {
            iter: v.into_iter(),
            buf: VecDeque::new(),
        }
    }

    pub fn is_empty( &mut self ) -> bool {
        self.peek().is_none()
    }

    pub fn peek( &mut self ) -> Option<&T> {
        if self.buf.len() == 0 {
            self.buf.push_back( self.iter.next()? );
        }

        self.buf.front()
    }

    pub fn peek_ahead( &mut self, distance: usize ) -> Option<&T> {
        if distance == 0 {
            self.peek()
        } else if distance < self.buf.len() {
            self.buf.get( distance )
        } else {
            for _ in 0 ..= ( distance - self.buf.len() ) {
                self.buf.push_back( self.iter.next()? );
            }

            self.buf.get( distance )
        }
    }

    pub fn consume( &mut self ) -> Option<T> {
        if self.buf.len() > 0 {
            self.buf.pop_front()
        } else {
            self.iter.next()
        }
    }
}
