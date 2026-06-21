// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/upgrade_auth_fixed.rs.
contract UnprotectedUpgradeFixed {
    address public implementation;
    address public immutable admin;

    constructor() {
        admin = msg.sender;
    }

    // FIX: gate the upgrade entrypoint on the admin, so only the trusted owner can
    // repoint the implementation.
    function upgradeTo(address newImplementation) external {
        require(msg.sender == admin, "not admin");
        implementation = newImplementation;
    }

    fallback() external payable {
        address impl = implementation;
        assembly {
            calldatacopy(0, 0, calldatasize())
            let result := delegatecall(gas(), impl, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch result
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }

    receive() external payable {}
}
