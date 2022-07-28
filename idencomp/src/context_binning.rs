use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::Deref;

use itertools::Itertools;
use rayon::prelude::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
    ParallelSliceMut,
};

use crate::context::{Context, ContextMergeCost};
use crate::context_spec::ContextSpec;
use crate::model::Model;
use crate::progress::{DummyProgressNotifier, ProgressNotifier};

#[must_use]
pub fn bin_contexts_with_model(model: &Model, options: &ContextBinningOptions) -> ContextTree {
    let complex_contexts = model.as_complex_contexts();
    for ctx in &complex_contexts {
        if ctx.specs().len() != 1 {
            panic!("Invalid context spec number: {}", ctx.specs().len());
        }
    }

    let contexts: Vec<(ContextSpec, Context)> = complex_contexts
        .into_iter()
        .map(|x| (x.specs.into_iter().next().unwrap(), x.context))
        .collect();

    bin_contexts_with_keys(contexts, options)
}

#[must_use]
pub fn bin_contexts_with_keys<I>(contexts: I, options: &ContextBinningOptions) -> ContextTree
where
    I: IntoIterator<Item = (ContextSpec, Context)>,
{
    let mut contexts: Vec<(ContextSpec, Context)> = contexts.into_iter().collect();

    let pre_binned = if options.pre_binning_num < contexts.len() {
        contexts.sort_by(|(_, ctx_1), (_, ctx_2)| ctx_2.context_prob.cmp(&ctx_1.context_prob));

        let (spec, mut context_binned) = contexts.pop().unwrap();
        let mut specs_binned = vec![spec];

        while options.pre_binning_num < contexts.len() + 1 {
            let (spec, context) = contexts.pop().unwrap();
            specs_binned.push(spec);
            context_binned = context_binned.merge_with(&context);
        }

        let node = ContextNode::new_leaf_multi(specs_binned, context_binned);
        Some(node)
    } else {
        None
    };

    let mut nodes: Vec<ContextNode> = contexts
        .into_iter()
        .map(|(key, context)| ContextNode::new_leaf(key, context))
        .collect();

    if let Some(pre_binned) = pre_binned {
        nodes.push(pre_binned);
    }

    bin_contexts_nodes(nodes, options)
}

#[must_use]
fn bin_contexts_nodes(mut nodes: Vec<ContextNode>, options: &ContextBinningOptions) -> ContextTree {
    let input_length = nodes.len();

    let initial_indices: Vec<(usize, usize)> = (0..nodes.len()).tuple_combinations().collect();
    let mut initial_elements = Vec::with_capacity(initial_indices.len());
    initial_indices
        .into_par_iter()
        .map(|(i, j)| QueuedNode::from_merge(&nodes, i, j))
        .collect_into_vec(&mut initial_elements);
    initial_elements.par_sort_unstable_by(|a, b| b.cmp(a));

    let mut available = vec![true; input_length];
    let mut queue: BinaryHeap<QueuedNode> = BinaryHeap::from(initial_elements);

    options
        .progress_notifier
        .set_iter_num((input_length - 1) as u64);
    for _ in 1..input_length {
        let current = loop {
            let current = queue.pop().unwrap();
            let (left_child, right_child) = current.children();
            if available[left_child] && available[right_child] {
                break current;
            }
        };

        let (left_child, right_child) = current.children();
        available[left_child] = false;
        available[right_child] = false;

        nodes.push(current.context_node(&nodes));
        let current_index = nodes.len() - 1;

        let new_items: Vec<QueuedNode> = available
            .par_iter()
            .enumerate()
            .filter_map(|(i, &is_available)| {
                if is_available {
                    Some(QueuedNode::from_merge(&nodes, i, current_index))
                } else {
                    None
                }
            })
            .collect();
        queue.extend(new_items);

        available.push(true);
        options.progress_notifier.inc_iter();
    }

    ContextTree::new(nodes)
}

#[derive(Debug)]
pub struct ContextBinningOptions {
    progress_notifier: Box<dyn ProgressNotifier>,
    pre_binning_num: usize,
}

impl ContextBinningOptions {
    pub fn builder() -> ContextBinningOptionsBuilder {
        ContextBinningOptionsBuilder::new()
    }
}

impl Default for ContextBinningOptions {
    fn default() -> Self {
        Self::builder().build()
    }
}

pub struct ContextBinningOptionsBuilder {
    progress_notifier: Box<dyn ProgressNotifier>,
    pre_binning_num: usize,
}

impl ContextBinningOptionsBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            progress_notifier: Box::new(DummyProgressNotifier),
            pre_binning_num: usize::MAX,
        }
    }

    pub fn progress_notifier(mut self, progress_notifier: Box<dyn ProgressNotifier>) -> Self {
        self.progress_notifier = progress_notifier;
        self
    }

    pub fn pre_binning_num(mut self, pre_binning_num: usize) -> Self {
        self.pre_binning_num = pre_binning_num;
        self
    }

    #[must_use]
    pub fn build(self) -> ContextBinningOptions {
        ContextBinningOptions {
            progress_notifier: self.progress_notifier,
            pre_binning_num: self.pre_binning_num,
        }
    }
}

impl Default for ContextBinningOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct QueuedNode {
    merge_cost: ContextMergeCost,
    left_index: u32,
    right_index: u32,
}

impl QueuedNode {
    #[must_use]
    fn from_merge(nodes: &[ContextNode], left_index: usize, right_index: usize) -> Self {
        let context_node = Self::make_context_node(nodes, left_index, right_index);

        Self {
            merge_cost: context_node.merge_cost(),
            left_index: left_index as u32,
            right_index: right_index as u32,
        }
    }

    #[must_use]
    fn children(&self) -> (usize, usize) {
        (self.left_index as usize, self.right_index as usize)
    }

    #[must_use]
    fn context_node(&self, nodes: &[ContextNode]) -> ContextNode {
        Self::make_context_node(nodes, self.left_index as usize, self.right_index as usize)
    }

    #[must_use]
    fn make_context_node(
        nodes: &[ContextNode],
        left_index: usize,
        right_index: usize,
    ) -> ContextNode {
        let left = &nodes[left_index];
        let right = &nodes[right_index];

        ContextNode::new_from_merge(left.context(), right.context(), left_index, right_index)
    }
}

impl PartialEq for QueuedNode {
    fn eq(&self, other: &Self) -> bool {
        self.merge_cost == other.merge_cost
    }
}

impl Eq for QueuedNode {}

impl PartialOrd for QueuedNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.merge_cost.partial_cmp(&self.merge_cost)
    }
}

impl Ord for QueuedNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.merge_cost.cmp(&self.merge_cost)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ComplexContext {
    pub(crate) specs: Vec<ContextSpec>,
    pub(crate) context: Context,
}

impl ComplexContext {
    pub fn new<T: Into<Vec<ContextSpec>>>(specs: T, context: Context) -> Self {
        let mut specs = specs.into();
        specs.sort();
        assert!(specs.iter().all_unique());

        Self { specs, context }
    }

    pub fn with_single_spec(spec: ContextSpec, context: Context) -> Self {
        let specs = vec![spec];

        Self { specs, context }
    }

    pub fn specs(&self) -> &Vec<ContextSpec> {
        &self.specs
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn into_spec_and_context(self) -> (Vec<ContextSpec>, Context) {
        (self.specs, self.context)
    }
}

impl PartialOrd for ComplexContext {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.specs.partial_cmp(&other.specs)
    }
}

impl Ord for ComplexContext {
    fn cmp(&self, other: &Self) -> Ordering {
        self.specs.cmp(&other.specs)
    }
}

struct IndexedContextNode<'a> {
    vec: &'a Vec<ContextNode>,
    index: usize,
}

impl<'a> IndexedContextNode<'a> {
    pub fn new(vec: &'a Vec<ContextNode>, index: usize) -> Self {
        Self { vec, index }
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

impl<'a> Deref for IndexedContextNode<'a> {
    type Target = ContextNode;

    fn deref(&self) -> &Self::Target {
        &self.vec[self.index]
    }
}

impl<'a> PartialEq for IndexedContextNode<'a> {
    fn eq(&self, other: &Self) -> bool {
        (*self).merge_cost().eq(&(*other).merge_cost())
    }
}

impl<'a> Eq for IndexedContextNode<'a> {}

impl<'a> PartialOrd for IndexedContextNode<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (*other).merge_cost().partial_cmp(&(*self).merge_cost())
    }
}

impl<'a> Ord for IndexedContextNode<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        (*other).merge_cost().cmp(&(*self).merge_cost())
    }
}

#[derive(Debug, Clone)]
pub struct ContextTree {
    vec: Vec<ContextNode>,
}

impl ContextTree {
    #[must_use]
    pub fn new<T: Into<Vec<ContextNode>>>(vec: T) -> Self {
        let vec = vec.into();
        assert!(!vec.is_empty());

        Self { vec }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.vec.len()
    }

    #[must_use]
    pub fn nodes(&self) -> &Vec<ContextNode> {
        &self.vec
    }

    #[must_use]
    pub fn traverse(self, num_contexts: usize) -> Vec<ComplexContext> {
        assert!(num_contexts > 0);

        let mut queue: BinaryHeap<IndexedContextNode> = BinaryHeap::new();
        queue.push(IndexedContextNode::new(&self.vec, self.vec.len() - 1));
        let mut result = Vec::new();

        while !queue.is_empty() && result.len() + queue.len() < num_contexts {
            let node = queue.pop().unwrap();
            let index = node.index();

            match *node {
                ContextNode::Leaf { .. } => {
                    result.push(self.combine_contexts(index));
                }
                ContextNode::Node {
                    left_child,
                    right_child,
                    ..
                } => {
                    queue.push(IndexedContextNode::new(&self.vec, left_child));
                    queue.push(IndexedContextNode::new(&self.vec, right_child));
                }
            }
        }

        while !queue.is_empty() {
            let elem = queue.pop().unwrap();
            result.push(self.combine_contexts(elem.index()));
        }

        result
    }

    fn combine_contexts(&self, index: usize) -> ComplexContext {
        let mut specs = Vec::new();
        self.traverse_and_combine(index, &mut specs);
        let context = self.vec[index].context().clone();

        ComplexContext::new(specs, context)
    }

    fn traverse_and_combine(&self, index: usize, specs: &mut Vec<ContextSpec>) {
        let node = &self.vec[index];
        match node {
            ContextNode::Leaf {
                specs: node_specs, ..
            } => {
                specs.extend(node_specs);
            }
            ContextNode::Node {
                left_child,
                right_child,
                ..
            } => {
                self.traverse_and_combine(*left_child, specs);
                self.traverse_and_combine(*right_child, specs);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextNode {
    Leaf {
        specs: Vec<ContextSpec>,
        context: Context,
    },
    Node {
        context: Context,
        merge_cost: ContextMergeCost,
        left_child: usize,
        right_child: usize,
    },
}

impl ContextNode {
    #[must_use]
    pub(crate) fn new_from_merge(
        left: &Context,
        right: &Context,
        left_index: usize,
        right_index: usize,
    ) -> Self {
        let context = left.merge_with(right);
        let merge_cost = Context::merge_cost(&context, left, right);

        Self::new_node(context, merge_cost, left_index, right_index)
    }

    #[must_use]
    pub(crate) fn new_leaf(spec: ContextSpec, context: Context) -> Self {
        Self::Leaf {
            specs: vec![spec],
            context,
        }
    }

    #[must_use]
    pub(crate) fn new_leaf_multi<T>(specs: T, context: Context) -> Self
    where
        T: Into<Vec<ContextSpec>>,
    {
        Self::Leaf {
            specs: specs.into(),
            context,
        }
    }

    #[must_use]
    pub(crate) fn new_node(
        context: Context,
        merge_cost: ContextMergeCost,
        left_child: usize,
        right_child: usize,
    ) -> Self {
        Self::Node {
            context,
            merge_cost,
            left_child,
            right_child,
        }
    }

    #[must_use]
    pub fn context(&self) -> &Context {
        match self {
            ContextNode::Leaf { context, .. } => context,
            ContextNode::Node { context, .. } => context,
        }
    }

    #[must_use]
    pub fn merge_cost(&self) -> ContextMergeCost {
        match self {
            ContextNode::Leaf { .. } => ContextMergeCost::ZERO,
            ContextNode::Node { merge_cost, .. } => *merge_cost,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::_internal_test_data::RANDOM_200_CTX_Q_SCORE_MODEL;
    use crate::context::Context;
    use crate::context_binning::{
        bin_contexts_with_keys, bin_contexts_with_model, ComplexContext, ContextBinningOptions,
        ContextMergeCost, ContextNode, ContextTree,
    };
    use crate::context_spec::{ContextSpec, ContextSpecType};
    use crate::model::{Model, ModelType};

    fn spec(i: u8) -> ContextSpec {
        ContextSpec::new(i as u32)
    }

    #[test]
    fn test_bin_single_context() {
        let context = Context::new_from(0.75, [0.0, 0.5, 0.3, 0.2]);
        let contexts = [(spec(0), context.clone())];

        let binned = bin_contexts_with_keys(contexts, &Default::default());

        assert_eq!(binned.size(), 1);
        assert_eq!(binned.nodes()[0], ContextNode::new_leaf(spec(0), context));
    }

    #[test]
    fn test_bin_model_single_context() {
        let context = Context::new_from(1.0, [0.0, 0.5, 0.3, 0.2, 0.0]);
        let contexts = [ComplexContext::with_single_spec(spec(0), context.clone())];
        let model =
            Model::with_model_and_spec_type(ModelType::Acids, ContextSpecType::Dummy, contexts);

        let binned = bin_contexts_with_model(&model, &Default::default());

        assert_eq!(binned.size(), 1);
        assert_eq!(binned.nodes()[0], ContextNode::new_leaf(spec(0), context));
    }

    #[test]
    fn test_bin_two_contexts() {
        let context1 = Context::new_from(0.75, [0.0, 0.5, 0.3, 0.2]);
        let context2 = Context::new_from(0.25, [0.25, 0.5, 0.125, 0.125]);
        let contexts = [(spec(1), context1.clone()), (spec(2), context2.clone())];

        let binned = bin_contexts_with_keys(contexts, &Default::default());

        assert_eq!(binned.size(), 3);
        assert_eq!(binned.nodes()[0], ContextNode::new_leaf(spec(1), context1));
        assert_eq!(binned.nodes()[1], ContextNode::new_leaf(spec(2), context2));
        let expected_context = Context::new_from(1.0, [0.0625, 0.5, 0.25625, 0.18125]);
        assert_eq!(
            binned.nodes()[2],
            ContextNode::new_node(expected_context, 0.14835548.into(), 0, 1)
        );
    }

    #[test]
    fn test_prebinning() {
        let context1 = Context::new_from(0.4, [1.0, 0.0, 0.0, 0.0]);
        let context2 = Context::new_from(0.3, [1.0, 0.0, 0.0, 0.0]);
        let context3 = Context::new_from(0.3, [0.25, 0.25, 0.25, 0.25]);
        let contexts = [
            (spec(1), context1.clone()),
            (spec(2), context2),
            (spec(3), context3),
        ];

        let options = ContextBinningOptions::builder().pre_binning_num(2).build();
        let binned = bin_contexts_with_keys(contexts, &options);

        assert_eq!(binned.size(), 3);
        assert_eq!(binned.nodes()[0], ContextNode::new_leaf(spec(1), context1));
        let expected_context_binned = Context::new_from(0.6, [0.625, 0.125, 0.125, 0.125]);
        assert_eq!(
            binned.nodes()[1],
            ContextNode::new_leaf_multi([spec(3), spec(2)], expected_context_binned)
        );
        let expected_context_root = Context::new_from(1.0, [0.775, 0.075, 0.075, 0.075]);
        assert_eq!(
            binned.nodes()[2],
            ContextNode::new_node(expected_context_root, 0.19653243.into(), 0, 1)
        );
    }

    #[test]
    fn test_bin_multiple_contexts() {
        let context1 = Context::new_from(0.27, [0.1, 0.8, 0.0, 0.1]);
        let context2 = Context::new_from(0.03, [0.4, 0.1, 0.2, 0.3]);
        let context3 = Context::new_from(0.21, [0.0, 0.2, 0.7, 0.1]);
        let context4 = Context::new_from(0.02, [0.5, 0.0, 0.0, 0.5]);
        let context5 = Context::new_from(0.08, [0.3, 0.2, 0.2, 0.3]);
        let context6 = Context::new_from(0.21, [0.2, 0.2, 0.5, 0.1]);
        let context7 = Context::new_from(0.03, [0.5, 0.3, 0.2, 0.0]);
        let context8 = Context::new_from(0.15, [0.2, 0.5, 0.0, 0.3]);
        let contexts = [
            (spec(1), context1.clone()),
            (spec(2), context2.clone()),
            (spec(3), context3.clone()),
            (spec(4), context4.clone()),
            (spec(5), context5.clone()),
            (spec(6), context6.clone()),
            (spec(7), context7.clone()),
            (spec(8), context8.clone()),
        ];

        let binned = bin_contexts_with_keys(contexts, &Default::default());

        assert_eq!(binned.size(), 15);
        assert_eq!(binned.nodes()[0], ContextNode::new_leaf(spec(1), context1));
        assert_eq!(binned.nodes()[1], ContextNode::new_leaf(spec(2), context2));
        assert_eq!(binned.nodes()[2], ContextNode::new_leaf(spec(3), context3));
        assert_eq!(binned.nodes()[3], ContextNode::new_leaf(spec(4), context4));
        assert_eq!(binned.nodes()[4], ContextNode::new_leaf(spec(5), context5));
        assert_eq!(binned.nodes()[5], ContextNode::new_leaf(spec(6), context6));
        assert_eq!(binned.nodes()[6], ContextNode::new_leaf(spec(7), context7));
        assert_eq!(binned.nodes()[7], ContextNode::new_leaf(spec(8), context8));

        let expected_context =
            Context::new_from(0.110000, [0.327273, 0.172727, 0.200000, 0.300000]);
        assert_eq!(
            binned.nodes()[8],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.001480), 1, 4)
        );

        let expected_context =
            Context::new_from(0.130000, [0.353846, 0.146154, 0.169231, 0.330769]);
        assert_eq!(
            binned.nodes()[9],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.012127), 3, 8)
        );

        let expected_context =
            Context::new_from(0.240000, [0.237500, 0.212500, 0.462500, 0.087500]);
        assert_eq!(
            binned.nodes()[10],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.015100), 5, 6)
        );

        let expected_context =
            Context::new_from(0.420000, [0.135714, 0.692857, 0.000000, 0.171429]);
        assert_eq!(
            binned.nodes()[11],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.029733), 0, 7)
        );

        let expected_context =
            Context::new_from(0.370000, [0.278378, 0.189189, 0.359459, 0.172973]);
        assert_eq!(
            binned.nodes()[12],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.040356), 9, 10)
        );

        let expected_context =
            Context::new_from(0.580000, [0.177586, 0.193103, 0.482759, 0.146552]);
        assert_eq!(
            binned.nodes()[13],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.092793), 2, 12)
        );

        let expected_context =
            Context::new_from(1.000000, [0.160000, 0.403000, 0.280000, 0.157000]);
        assert_eq!(
            binned.nodes()[14],
            ContextNode::new_node(expected_context, ContextMergeCost::new(0.331311), 11, 13)
        );
    }

    #[test]
    fn test_bin_bigger_model() {
        let tree = bin_contexts_with_model(&RANDOM_200_CTX_Q_SCORE_MODEL, &Default::default());
        assert_eq!(tree.size(), 399);
    }

    #[test]
    fn context_tree_traverse() {
        let spec1 = spec(1);
        let context1 = Context::new_from(0.69, [0.1, 0.8, 0.0, 0.1]);
        let spec2 = spec(2);
        let context2 = Context::new_from(0.31, [0.4, 0.1, 0.2, 0.3]);

        let nodes = [
            ContextNode::new_leaf(spec1, context1.clone()),
            ContextNode::new_leaf(spec2, context2.clone()),
            ContextNode::new_from_merge(&context1, &context2, 0, 1),
        ];

        let tree = ContextTree::new(nodes.clone());
        let vec = tree.traverse(2);
        assert_eq!(
            vec,
            [
                ComplexContext::new([spec1], context1),
                ComplexContext::new([spec2], context2),
            ]
        );

        let context_combined = Context::new_from(1.0, [0.193, 0.583, 0.062, 0.162]);
        let tree = ContextTree::new(nodes);
        let vec = tree.traverse(1);
        assert_eq!(vec, [ComplexContext::new([spec1, spec2], context_combined)]);
    }
}
