// SPDX-License-Identifier: Apache 2.0
pragma solidity ^0.8.28;

import "hardhat/console.sol";

contract CounterByAddress {
    mapping(address => uint256) public counter;
    address trustedForwarderAddress;

    function _msgSender() internal view returns (address sender) {
        if (msg.sender == trustedForwarderAddress && msg.data.length >= 20) {
            assembly {
                sender := shr(96, calldataload(sub(calldatasize(), 20)))
            }
        } else {
            sender = msg.sender;
        }
    }

    function increment() public {
        counter[_msgSender()]++;
    }

    function getCounter(address addr) public view returns (uint256) {
        return counter[addr];
    }

    function setTrustedForwarderAddress(address addr) public {
        trustedForwarderAddress = addr;
    }

    function getTrustedForwarderAddress() public view returns (address) {
        return trustedForwarderAddress;
    }
}
