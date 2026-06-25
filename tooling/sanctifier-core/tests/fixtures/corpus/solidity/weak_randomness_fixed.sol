// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/weak_randomness_fixed.rs.
contract WeakRandomnessFixed {
    address public winner;
    address public immutable vrf;

    constructor(address vrf_) {
        vrf = vrf_;
    }

    // FIX: randomness comes from a trusted external VRF oracle, not from
    // block/ledger values, so it cannot be predicted or ground by a producer.
    function draw(address[] calldata players, uint256 randomWord) external {
        require(msg.sender == vrf, "only vrf");
        winner = players[randomWord % players.length];
    }
}
