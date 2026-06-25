// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/unbounded_loop_fixed.rs.
contract UnboundedLoopFixed {
    mapping(address => uint256) public owed;

    function enroll() external {
        owed[msg.sender] += 1 ether;
    }

    // FIX: pull-payment. No external call in a loop — each recipient withdraws its
    // own share, so a single bad recipient cannot block everyone else.
    function withdraw() external {
        uint256 amount = owed[msg.sender];
        require(amount > 0, "nothing owed");
        owed[msg.sender] = 0;
        (bool ok, ) = msg.sender.call{value: amount}("");
        require(ok, "transfer failed");
    }
}
