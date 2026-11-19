use zeyn::ReorderRequest;
use zeyn::ReorderRequest as Move;
use zeyn::rank::{Rank, ReorderRequest};

fn make() -> zeyn::ReorderRequest<u64> {
    zeyn::ReorderRequest::new(1, None, None)
}
