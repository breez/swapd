use std::collections::HashMap;

use bitcoin::BlockHash;

use super::{BlockHeader, ChainError};

#[derive(Clone)]
struct BlockInfo {
    header: BlockHeader,
    next: Option<BlockHash>,
}

#[derive(Clone)]
pub(super) struct Chain {
    tip: BlockHash,
    base: BlockHash,
    blocks: HashMap<BlockHash, BlockInfo>,
}

impl Chain {
    pub(super) fn new(tip: BlockHeader) -> Self {
        let mut chain = Chain {
            tip: tip.hash,
            base: tip.hash,
            blocks: HashMap::new(),
        };
        chain.blocks.insert(
            tip.hash,
            BlockInfo {
                header: tip,
                next: None,
            },
        );
        chain
    }

    pub(super) fn append(&mut self, new_tip: BlockHeader) -> Result<(), ChainError> {
        if new_tip.prev != self.tip {
            return Err(ChainError::InvalidChain);
        }

        let old_tip = self
            .blocks
            .get_mut(&self.tip)
            .expect("chain does not contain its own tip");
        old_tip.next = Some(new_tip.hash);
        self.tip = new_tip.hash;
        self.blocks.insert(
            new_tip.hash,
            BlockInfo {
                header: new_tip,
                next: None,
            },
        );
        Ok(())
    }

    pub(super) fn base(&self) -> BlockHeader {
        self.blocks
            .get(&self.base)
            .expect("chain does not contain its own base")
            .header
            .clone()
    }

    pub(super) fn contains_block(&self, hash: &BlockHash) -> bool {
        self.blocks.contains_key(hash)
    }

    #[allow(unused)]
    pub(super) fn get_block(&self, hash: &BlockHash) -> Result<BlockHeader, ChainError> {
        match self.blocks.get(hash) {
            Some(block) => Ok(block.header.clone()),
            None => Err(ChainError::BlockNotFound),
        }
    }

    pub(super) fn prepend(&mut self, base: BlockHeader) -> Result<(), ChainError> {
        let old_base = self
            .blocks
            .get(&self.base)
            .expect("chain doesn't contain its own base");
        if old_base.header.prev != base.hash {
            return Err(ChainError::InvalidChain);
        }

        self.base = base.hash;
        self.blocks.insert(
            base.hash,
            BlockInfo {
                header: base,
                next: Some(old_base.header.hash),
            },
        );
        Ok(())
    }

    #[allow(unused)]
    pub(super) fn rebase(&mut self, other: &Chain) -> Result<(), ChainError> {
        let mut next_block = self
            .blocks
            .get(&self.base)
            .expect("chain doesn't contain its own base")
            .header
            .clone();
        loop {
            if next_block.hash == other.base {
                break;
            }

            let current_block = other.get_block(&next_block.prev)?;
            self.prepend(current_block.clone())?;
            next_block = current_block;
        }

        Ok(())
    }

    pub(super) fn retip(&mut self, other: &Chain) -> Result<(), ChainError> {
        let mut next_block_header = {
            let tip = match self.blocks.get_mut(&other.base) {
                Some(block) => block,
                None => return Err(ChainError::BlockNotFound),
            };

            let next_block_header = tip.next;
            tip.next = None;
            self.tip = tip.header.hash;
            next_block_header
        };

        while let Some(current_block_header) = next_block_header {
            next_block_header = self
                .blocks
                .get(&current_block_header)
                .expect("chain misses expected block")
                .next;
            self.blocks.remove(&current_block_header);
        }

        for block in other.iter_forwards().skip(1) {
            self.append(block.clone())?;
        }

        Ok(())
    }

    pub(super) fn iter_forwards(&self) -> ForwardChainIterator {
        ForwardChainIterator::new(self)
    }

    pub(super) fn iter_backwards(&self) -> BackwardChainIterator {
        BackwardChainIterator::new(self)
    }

    pub(super) fn tip(&self) -> BlockHeader {
        self.blocks
            .get(&self.tip)
            .expect("chain does not contain its own tip")
            .header
            .clone()
    }
}

pub(super) struct ForwardChainIterator<'a> {
    chain: &'a Chain,
    current: Option<&'a BlockInfo>,
}

impl<'a> ForwardChainIterator<'a> {
    fn new(chain: &'a Chain) -> Self {
        let current = chain
            .blocks
            .get(&chain.base)
            .expect("chain does not contain its own base");
        ForwardChainIterator {
            chain,
            current: Some(current),
        }
    }
}

impl<'a> Iterator for ForwardChainIterator<'a> {
    type Item = &'a BlockHeader;
    fn next(&mut self) -> Option<&'a BlockHeader> {
        let current = self.current?;
        self.current = match current.next {
            Some(next) => Some(
                self.chain
                    .blocks
                    .get(&next)
                    .expect("chain does not contain expected next block"),
            ),
            None => None,
        };

        Some(&current.header)
    }
}

pub(super) struct BackwardChainIterator<'a> {
    chain: &'a Chain,
    current: Option<&'a BlockInfo>,
}

impl<'a> BackwardChainIterator<'a> {
    fn new(chain: &'a Chain) -> Self {
        let current = chain
            .blocks
            .get(&chain.tip)
            .expect("chain doesn't contain its own tip");
        BackwardChainIterator {
            chain,
            current: Some(current),
        }
    }
}

impl<'a> Iterator for BackwardChainIterator<'a> {
    type Item = &'a BlockHeader;
    fn next(&mut self) -> Option<&'a BlockHeader> {
        let current = self.current?;
        self.current = match current.header.hash == self.chain.base {
            true => None,
            false => Some(
                self.chain
                    .blocks
                    .get(&current.header.prev)
                    .expect("chain does not contain expected prev block"),
            ),
        };

        Some(&current.header)
    }
}

impl TryFrom<Vec<super::types::BlockHeader>> for Chain {
    type Error = ChainError;

    fn try_from(headers: Vec<super::types::BlockHeader>) -> Result<Self, Self::Error> {
        if headers.is_empty() {
            return Err(ChainError::EmptyChain);
        }

        let tip_header = headers[0].clone();
        let mut chain = Chain::new(tip_header);
        for header in headers.into_iter().skip(1) {
            chain.prepend(header)?;
        }
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::{hashes::Hash, BlockHash};

    use crate::chain::{BlockHeader, ChainError};

    use super::Chain;

    fn hash(height: u8) -> BlockHash {
        BlockHash::from_slice(&[height; 32]).unwrap()
    }

    fn header(height: u8) -> BlockHeader {
        BlockHeader {
            hash: hash(height),
            height: height as u64,
            prev: hash(height - 1),
        }
    }

    #[test]
    fn test_new_has_tip_base_and_block() {
        let header = header(1);
        let chain = Chain::new(header.clone());
        assert_eq!(chain.base().hash, header.hash);
        assert_eq!(chain.tip().hash, header.hash);
        assert!(chain.contains_block(&header.hash));
        assert_eq!(chain.get_block(&header.hash).unwrap(), header);
    }

    #[test]
    fn test_append_success() {
        let base = header(1);
        let new_tip = header(2);
        let mut chain = Chain::new(base.clone());
        chain.append(new_tip.clone()).unwrap();
        assert_eq!(chain.tip().hash, new_tip.hash);
        assert_eq!(chain.base().hash, base.hash);
        assert_eq!(chain.get_block(&new_tip.hash).unwrap(), new_tip);
    }

    #[test]
    fn test_append_failure() {
        let base = header(1);
        let new_tip = header(3);
        let mut chain = Chain::new(base.clone());
        let result = chain.append(new_tip.clone());
        assert!(matches!(result, Err(ChainError::InvalidChain)));
    }

    #[test]
    fn test_prepend_success() {
        let tip = header(2);
        let new_base = header(1);
        let mut chain = Chain::new(tip.clone());
        chain.prepend(new_base.clone()).unwrap();
        assert_eq!(chain.tip().hash, tip.hash);
        assert_eq!(chain.base().hash, new_base.hash);
        assert_eq!(chain.get_block(&new_base.hash).unwrap(), new_base);
    }

    #[test]
    fn test_prepend_failure() {
        let tip = header(3);
        let new_base = header(1);
        let mut chain = Chain::new(tip.clone());
        let result = chain.prepend(new_base.clone());
        assert!(matches!(result, Err(ChainError::InvalidChain)));
    }

    #[test]
    fn test_rebase_success() {
        let base = Chain::try_from(vec![header(2), header(1)]).unwrap();
        let mut chain = Chain::try_from(vec![header(3), header(2)]).unwrap();
        chain.rebase(&base).unwrap();
        let blocks: Vec<_> = chain.iter_forwards().cloned().collect();
        assert_eq!(blocks[0], header(1));
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], header(3));

        let blocks: Vec<_> = chain.iter_backwards().cloned().collect();
        assert_eq!(blocks[0], header(3));
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], header(1));
    }

    #[test]
    fn test_rebase_reorg_success() {
        let base = Chain::try_from(vec![header(3), header(2), header(1)]).unwrap();
        let mut reorg_header = header(3);
        reorg_header.hash = hash(4);
        let mut chain = Chain::try_from(vec![reorg_header.clone(), header(2)]).unwrap();
        chain.rebase(&base).unwrap();
        let blocks: Vec<_> = chain.iter_forwards().cloned().collect();
        assert_eq!(blocks[0], header(1));
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], reorg_header.clone());

        let blocks: Vec<_> = chain.iter_backwards().cloned().collect();
        assert_eq!(blocks[0], reorg_header);
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], header(1));
    }

    #[test]
    fn test_retip_success() {
        let mut chain = Chain::try_from(vec![header(2), header(1)]).unwrap();
        let tip = Chain::try_from(vec![header(3), header(2)]).unwrap();
        chain.retip(&tip).unwrap();
        let blocks: Vec<_> = chain.iter_forwards().cloned().collect();
        assert_eq!(blocks[0], header(1));
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], header(3));

        let blocks: Vec<_> = chain.iter_backwards().cloned().collect();
        assert_eq!(blocks[0], header(3));
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], header(1));
    }

    #[test]
    fn test_retip_reorg_success() {
        let mut chain = Chain::try_from(vec![header(3), header(2), header(1)]).unwrap();
        let mut reorg_header = header(3);
        reorg_header.hash = hash(4);
        let tip = Chain::try_from(vec![reorg_header.clone(), header(2)]).unwrap();
        chain.retip(&tip).unwrap();
        let blocks: Vec<_> = chain.iter_forwards().cloned().collect();
        assert_eq!(blocks[0], header(1));
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], reorg_header.clone());

        let blocks: Vec<_> = chain.iter_backwards().cloned().collect();
        assert_eq!(blocks[0], reorg_header);
        assert_eq!(blocks[1], header(2));
        assert_eq!(blocks[2], header(1));
    }
}
