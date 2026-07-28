// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/confused_deputy_fixed.rs.
contract ConfusedDeputyFixed {
    address public owner;

    constructor() {
        owner = msg.sender;
    }

    // FIX: authenticate the direct caller (msg.sender), so an intermediate
    // contract cannot be used as a confused deputy.
    function setOwner(address newOwner) external {
        require(msg.sender == owner, "not owner");
        owner = newOwner;
    }
}
