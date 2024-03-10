(module
	(import "seal0" "call_chain_extension"
		(func $call_chain_extension (param i32 i32 i32 i32 i32) (result i32))
	)
	(import "seal0" "seal_input" (func $seal_input (param i32 i32)))
	(import "seal0" "seal_return" (func $seal_return (param i32 i32 i32)))
	(import "env" "memory" (memory 16 16))

	;; bytes [0, 4)  reserved for the length of input
	;; bytes [4, 38) reserved for the input to be read by the $seal_input function
	;;   - 4  bytes for extension method id
	;;   - 32 bytes for verifying key hash
	;;   - 2  bytes for empty proof and empty public input
	(data (i32.const 0) "\26")

	;; function for instantiating the contract
	(func (export "deploy"))

  ;; function for calling the contract
	(func (export "call")
		(call $seal_input
		  (i32.const 4) ;; input_ptr
		  (i32.const 0) ;; input_len_ptr
    )

		(call $call_chain_extension
			(i32.load (i32.const 4))	;; id
			(i32.const 8)				      ;; input_ptr
			(i32.const 34)          	;; input_len
			(i32.const 0)             ;; output_ptr
			(i32.const 0)				      ;; output_len_ptr
		)

		drop

		(call $seal_return (i32.const 0) (i32.const 0) (i32.const 0))
	)
)
