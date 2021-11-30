; `rustc` version info:
;      rustc 1.55.0-nightly (3e1c75c6e 2021-07-13)
;      binary: rustc
;      commit-hash: 3e1c75c6e25a4db968066bd2ef2dabc7c504d7ca
;      commit-date: 2021-07-13
;      host: x86_64-pc-windows-msvc
;      release: 1.55.0-nightly
;      LLVM version: 12.0.1
;
; Build command: `cargo clean && cargo rustc --release --package arbre_codegen -- --emit asm`

; === Object metadata

	.text
	.def	 @feat.00;
	.scl	3;
	.type	0;
	.endef
	.globl	@feat.00
.set @feat.00, 0
	.file	"arbre_codegen.6b64aeae-cgu.0"

; === <Foo as FooProxy>::do_something
	.def	 _ZN62_$LT$arbre_codegen..Foo$u20$as$u20$arbre_codegen..FooProxy$GT$12do_something17hb8d968ad5d2d48b5E;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",one_only,_ZN62_$LT$arbre_codegen..Foo$u20$as$u20$arbre_codegen..FooProxy$GT$12do_something17hb8d968ad5d2d48b5E
	.globl	_ZN62_$LT$arbre_codegen..Foo$u20$as$u20$arbre_codegen..FooProxy$GT$12do_something17hb8d968ad5d2d48b5E
	.p2align	4, 0x90
_ZN62_$LT$arbre_codegen..Foo$u20$as$u20$arbre_codegen..FooProxy$GT$12do_something17hb8d968ad5d2d48b5E:
	cmpl	$0, (%rcx)
	je	.LBB0_2
	retq
	.p2align	4, 0x90
.LBB0_2:
	jmp	.LBB0_2

; === fetch_static_static
	.def	 _ZN13arbre_codegen19fetch_static_static17ha8b3372cc8dca67bE;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",one_only,_ZN13arbre_codegen19fetch_static_static17ha8b3372cc8dca67bE
	.globl	_ZN13arbre_codegen19fetch_static_static17ha8b3372cc8dca67bE
	.p2align	4, 0x90
_ZN13arbre_codegen19fetch_static_static17ha8b3372cc8dca67bE:
	cmpl	$0, (%rcx)
	je	.LBB1_1
	retq
	.p2align	4, 0x90
.LBB1_1:
	jmp	.LBB1_1

; === fetch_static_dynamic

	.def	 _ZN13arbre_codegen20fetch_static_dynamic17h8b0ae576be4a79a3E;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",one_only,_ZN13arbre_codegen20fetch_static_dynamic17h8b0ae576be4a79a3E
	.globl	_ZN13arbre_codegen20fetch_static_dynamic17h8b0ae576be4a79a3E
	.p2align	4, 0x90
_ZN13arbre_codegen20fetch_static_dynamic17h8b0ae576be4a79a3E:
	cmpl	$0, (%rcx)
	je	.LBB2_1
	retq
	.p2align	4, 0x90
.LBB2_1:
	jmp	.LBB2_1

; === fetch_dynamic_static

	.def	 _ZN13arbre_codegen20fetch_dynamic_static17h3e77357ab60d6116E;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",one_only,_ZN13arbre_codegen20fetch_dynamic_static17h3e77357ab60d6116E
	.globl	_ZN13arbre_codegen20fetch_dynamic_static17h3e77357ab60d6116E
	.p2align	4, 0x90
_ZN13arbre_codegen20fetch_dynamic_static17h3e77357ab60d6116E:
.seh_proc _ZN13arbre_codegen20fetch_dynamic_static17h3e77357ab60d6116E
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$32, %rsp
	.seh_stackalloc 32
	.seh_endprologue
	movq	%rdx, %rbx
	movq	%rcx, %rsi
	callq	*24(%rdx)
	movq	%rax, %rcx
	movabsq	$-2189689144433176653, %r8
	movq	792(%rax), %rdi
	imulq	%r8, %rdi
	movabsq	$1117984489315730401, %rdx
	movq	%rdi, %rax
	mulq	%rdx
	shrq	%rdx
	movq	%rdx, %rax
	shlq	$5, %rax
	addq	%rdx, %rax
	subq	%rax, %rdi
	leaq	(%rdi,%rdi,2), %rax
	cmpq	%r8, (%rcx,%rax,8)
	jne	.LBB3_5
	movq	8(%rcx,%rax,8), %rdi
	movq	%rsi, %rcx
	movq	%rbx, %rdx
	callq	_ZN73_$LT$dyn$u20$arbre..fetch..Obj$u20$as$u20$arbre..fetch..DynObjConvert$GT$6to_dyn17h65de97845a78e061E
	testq	%rax, %rax
	je	.LBB3_5
	cmpl	$0, (%rsi,%rdi)
	je	.LBB3_3
	addq	$32, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	retq
	.p2align	4, 0x90
.LBB3_3:
	jmp	.LBB3_3
.LBB3_5:
	leaq	__unnamed_1(%rip), %rcx
	leaq	__unnamed_2(%rip), %r8
	movl	$43, %edx
	callq	_ZN4core9panicking5panic17h5cccc34aab9ab48dE
	ud2
	.seh_endproc

; === fetch_dynamic_dynamic

	.def	 _ZN13arbre_codegen21fetch_dynamic_dynamic17h615d311acd49b546E;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",one_only,_ZN13arbre_codegen21fetch_dynamic_dynamic17h615d311acd49b546E
	.globl	_ZN13arbre_codegen21fetch_dynamic_dynamic17h615d311acd49b546E
	.p2align	4, 0x90
_ZN13arbre_codegen21fetch_dynamic_dynamic17h615d311acd49b546E:
.seh_proc _ZN13arbre_codegen21fetch_dynamic_dynamic17h615d311acd49b546E
	pushq	%r14
	.seh_pushreg %r14
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$40, %rsp
	.seh_stackalloc 40
	.seh_endprologue
	movq	%rdx, %r14
	movq	%rcx, %rsi
	callq	*24(%rdx)
	movq	%rax, %rcx
	movabsq	$6663150091335034237, %r8
	movq	792(%rax), %rdi
	imulq	%r8, %rdi
	movabsq	$1117984489315730401, %rdx
	movq	%rdi, %rax
	mulq	%rdx
	shrq	%rdx
	movq	%rdx, %rax
	shlq	$5, %rax
	addq	%rdx, %rax
	subq	%rax, %rdi
	leaq	(%rdi,%rdi,2), %rax
	cmpq	%r8, (%rcx,%rax,8)
	jne	.LBB4_2
	movq	8(%rcx,%rax,8), %rdi
	movq	16(%rcx,%rax,8), %rbx
	movq	%rsi, %rcx
	movq	%r14, %rdx
	callq	_ZN73_$LT$dyn$u20$arbre..fetch..Obj$u20$as$u20$arbre..fetch..DynObjConvert$GT$6to_dyn17h65de97845a78e061E
	testq	%rax, %rax
	je	.LBB4_2
	addq	%rdi, %rsi
	movq	%rsi, %rcx
	movq	%rbx, %rax
	addq	$40, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	popq	%r14
	rex64 jmpq	*24(%rax)
.LBB4_2:
	leaq	__unnamed_1(%rip), %rcx
	leaq	__unnamed_2(%rip), %r8
	movl	$43, %edx
	callq	_ZN4core9panicking5panic17h5cccc34aab9ab48dE
	ud2
	.seh_endproc

; === Metadata

	.section	.rdata,"dr",one_only,__unnamed_1
__unnamed_1:
	.ascii	"called `Option::unwrap()` on a `None` value"

	.section	.rdata,"dr",one_only,__unnamed_3
__unnamed_3:
	.ascii	"..."  ; path redacted

	.section	.rdata,"dr",one_only,__unnamed_2
	.p2align	3
__unnamed_2:
	.quad	__unnamed_3
	.asciz	"N\000\000\000\000\000\000\000\037\001\000\000!\000\000"
