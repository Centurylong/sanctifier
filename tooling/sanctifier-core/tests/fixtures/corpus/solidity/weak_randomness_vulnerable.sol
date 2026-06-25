// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/weak_randomness_vulnerable.rs.
// Slither: weak-prng. Aderyn: weak-randomness.
contract WeakRandomnessVulnerable {
    address public winner;

    // VULN: randomness derived from on-chain, miner/validator-influenceable values
    // (block.timestamp, block.prevrandao). Fully predictable / grindable.
    function draw(address[] calldata players) external {
        uint256 r = uint256(
            keccak256(abi.encodePacked(block.timestamp, block.prevrandao))
        ) % players.length;
        winner = players[r];
    }
}
