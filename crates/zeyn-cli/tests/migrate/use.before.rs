use zeyn::rank::ReorderRequest;
use zeyn::rank::ReorderRequest as Move;
use zeyn::rank::{Rank, ReorderRequest};

fn make() -> zeyn::rank::ReorderRequest<u64> {
    zeyn::rank::ReorderRequest::new(1, None, None)
}
