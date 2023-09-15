#!/bin/bash

curl -H "Content-Type: application/json" \
	 -d '{"id":1, "jsonrpc":"2.0", "method": "alephNode_ready"}' \
	 http://localhost:${RPC_PORT} \
	 | grep '"result"\( \)*:\( \)*true'
