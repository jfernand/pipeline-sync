#![allow(unused)]
use crate::merkle::Node::Leaf;
use crate::merkle::Parity::{Even, Odd};
use crate::sha256::Sha256Hash;
use std::cmp::PartialEq;

#[derive(Debug, Clone)]
struct Tree {
    hash: Sha256Hash,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Tree {
    pub fn new(hash: Sha256Hash) -> Self {
        Tree {
            hash,
            left: None,
            right: None,
        }
    }

    pub(crate) fn insert(&mut self, hash: Sha256Hash) {
        if self.left.is_none() {
            self.left = Tree::new_tree(hash);
        } else if self.right.is_none() {
            self.right = Tree::new_tree(hash);
        } else {
            todo!()
        }
    }

    fn new_tree(hash: Sha256Hash) -> Option<Box<Node>> {
        Some(Box::new(Node::Tree(Tree::new(hash))))
    }
}

#[derive(Clone, Debug)]
enum Node {
    Null,
    Leaf(Sha256Hash),
    Tree(Tree),
}


#[derive(Clone, Debug)]
struct Root {
    root: Node,
    elements: u64,
    depth: u64,
    tail: Option<Node>,
}

impl Root {
    pub fn is_even(&self) -> bool {
        self.elements
            .is_multiple_of(2)
    }
}

#[derive(Clone, Debug, PartialEq, Copy)]
enum Parity {
    Odd,
    Even,
}

impl Parity {
    fn toggle(&mut self) {
        *self = match self {
            Odd => Even,
            Even => Odd,
        }
    }
}

impl Root {
    fn new() -> Self {
        Root {
            root: Node::Null,
            elements: 0,
            depth: 0,
            tail: None,
        }
    }

    fn insert(&mut self, hash: impl Into<Sha256Hash>) {
        let hash = hash.into();
        self.elements += 1; //We should really check to see if we exceed the maximum number of elements
        match &self.root {
            Node::Null => self.insert_leaf(hash),
            Leaf(left) => {
                self.root = Node::Tree(Tree {
                    hash: hash + *left,
                    left: Some(Box::new(self.root.clone())),
                    right: Some(Box::new(Leaf(hash))),
                })
            }
            // Node::Tree(tree) => {
            //     tree
            //         .insert(hash);
            // }
            Node::Tree(_tree) => {}
        }
    }

    fn insert_leaf(&mut self, hash: Sha256Hash) {
        let node = Leaf(hash);
        self.root = node.clone();
        self.tail = Some(
            self.root
                .clone(),
        ); // &root must live as long as self
    }

    fn insert_even(&self, _sha256hash: Sha256Hash) {
        todo!()
    }

    fn insert_odd(&self, _hash: Sha256Hash) {
        todo!()
    }
}

// () -> Null h:0 Special case 1
// (a) -> Leaf(a) h:1 Special case 2?
// (a+b) -> Tree(a+b, (a), (b)) 2^ h:2 // Becomes left of hash + new element
// (a+b+c) -> Tree(a+b+c, (a+b), (c)) h:3
// (a+b+c+d) -> (a+b), (c+d)

#[cfg(test)]
mod tests {
    use crate::merkle::Root; // This is probably obsolete
    use super::*;
    #[test]
    fn inner_tree() {
        let tree = Tree::new_tree("a".into());
        dbg!(&tree);
    }

    #[test]
    fn empty() {
        let mut root = Root::new();
        dbg!(&root.clone());
        assert!(root.is_even());

        root.insert("a");

        assert!(!root.is_even());
    }
}
