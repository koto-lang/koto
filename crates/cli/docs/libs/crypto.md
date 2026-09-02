# crypto

Cryptographic utilities for Koto: hashing, keyed hashing (HMAC), hex encoding,
authenticated symmetric encryption, and digital signatures.

## Features

The hashing functions, HMAC, hex encoding, and [`crypto.random_bytes`](#random_bytes)
are always available.

The encryption functions ([`crypto.encrypt`](#encrypt) and
[`crypto.decrypt`](#decrypt)) require the `encryption` feature, and the signing
functions ([`crypto.keypair`](#keypair), [`crypto.sign`](#sign), and
[`crypto.verify`](#verify)) require the `signing` feature. Both features are
enabled in the CLI by default.

## Binary data

Koto strings are UTF-8, so arbitrary binary data is represented in this module
as hex-encoded strings. Keys, nonces, digests, ciphertexts, and signatures are
all hex strings.

For example, a 32-byte key is a 64-character hex string, which can be generated
with [`crypto.random_bytes`](#random_bytes).

## blake2b

```kototype
|String| -> String
```

Returns the [BLAKE2b][blake2] digest of the input string, with a 512-bit output,
as a hex string.

### Example

```koto
print! crypto.blake2b 'hello'
check! e4cfa39a3d37be31c59609e807970799caa68a19bfaa15135f165085e01d41a65ba1e1b146aeb6bd0092b49eac214c103ccfa3a365954bbbe52f74a2b3620c94
```

## blake2s

```kototype
|String| -> String
```

Returns the [BLAKE2s][blake2] digest of the input string, with a 256-bit output,
as a hex string.

### Example

```koto
print! crypto.blake2s 'hello'
check! 19213bacc58dee6dbde3ceb9a47cbb330b3d86f8cca8997eb00be456f140ca25
```

## blake3

```kototype
|String| -> String
```

Returns the [BLAKE3][blake3] digest of the input string as a hex string.

### Example

```koto
print! crypto.blake3 ''
check! af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262
```

## sha256

```kototype
|String| -> String
```

Returns the [SHA-256][sha2] digest of the input string as a hex string.

### Example

```koto
print! crypto.sha256 'hello'
check! 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
```

## sha512

```kototype
|String| -> String
```

Returns the [SHA-512][sha2] digest of the input string as a hex string.

### Example

```koto
print! crypto.sha512 'hello'
check! 9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043
```

## sha1

```kototype
|String| -> String
```

Returns the [SHA-1][sha1] digest of the input string as a hex string.

SHA-1 is considered cryptographically broken and should not be used in new
systems; it's provided for interoperability with legacy data.

### Example

```koto
print! crypto.sha1 'hello'
check! aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d
```

## md5

```kototype
|String| -> String
```

Returns the [MD5][md5] digest of the input string as a hex string.

MD5 is considered cryptographically broken and should not be used in new
systems; it's provided for interoperability with legacy data.

### Example

```koto
print! crypto.md5 'hello'
check! 5d41402abc4b2a76b9719d911017c592
```

## hmac

```kototype
|String, String, String| -> String
```

Returns a keyed-hash message authentication code ([HMAC][hmac]) as a hex string.

The first argument is the hash algorithm to use, and must be one of:
`blake2b`, `blake2s`, `sha256`, `sha512`, `sha1`, or `md5`.

The second argument is the key, as a hex-encoded string.
Use [`crypto.hex_encode`](#hex_encode) to turn a text key into its hex
representation, or [`crypto.random_bytes`](#random_bytes) to generate a random key.

The third argument is the message to authenticate.

### Example

```koto
key = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f'
print! crypto.hmac 'blake2b', key, 'hello'
check! 32519f3c0d076330d5b9acadf44097f9f462bfc42955c4169866f96398d3632d2cea55f1a8207a36a2bf88eb364e8a4eada28f650f8ad2d27a78a25cee5ab531
```

## hex_encode

```kototype
|String| -> String
```

Returns the hex encoding of the input string's bytes.

### Example

```koto
print! crypto.hex_encode 'Hello'
check! 48656c6c6f
```

## hex_decode

```kototype
|String| -> List
```

Decodes a hex-encoded string, returning the decoded bytes as a list of numbers.

### Example

```koto
print! crypto.hex_decode '48656c6c6f'
check! [72, 101, 108, 108, 111]
```

## random_bytes

```kototype
|Number| -> String
```

Returns the specified number of cryptographically secure random bytes as a hex
string.

### Example

```koto
# 8 bytes are encoded as 16 hex characters
bytes = crypto.random_bytes 8
print! size bytes
check! 16
```

## encrypt

```kototype
|String, String| -> String
```

Encrypts the plaintext string using [ChaCha20-Poly1305][chacha20poly1305], an
authenticated encryption algorithm, and returns the result as a hex string.

The first argument is a 32-byte key, as a hex string (see
[`crypto.random_bytes`](#random_bytes)).

A fresh random nonce is generated for each call and prepended to the ciphertext,
so encrypting the same plaintext twice produces different results.

### Example

```koto
key = crypto.random_bytes 32
ciphertext = crypto.encrypt key, 'secret message'
print! crypto.decrypt key, ciphertext
check! secret message
```

## decrypt

```kototype
|String, String| -> String
```

Decrypts a hex-encoded ciphertext produced by [`crypto.encrypt`](#encrypt),
returning the plaintext string.

The first argument is the same 32-byte key that was used to encrypt the data.

### Example

```koto
key = crypto.random_bytes 32
ciphertext = crypto.encrypt key, 'secret message'
print! crypto.decrypt key, ciphertext
check! secret message
```

## keypair

```kototype
|| -> Map
```

Generates a new [Ed25519][ed25519] keypair, returning a map containing the
hex-encoded `secret` and `public` keys.

### Example

```koto
keypair = crypto.keypair()
signature = crypto.sign keypair.secret, 'important message'
print! crypto.verify keypair.public, 'important message', signature
check! true
```

## sign

```kototype
|String, String| -> String
```

Signs the message string using an [Ed25519][ed25519] secret key, returning the
signature as a hex string.

The first argument is the 32-byte secret key as a hex string (see
[`crypto.keypair`](#keypair)).

### Example

```koto
keypair = crypto.keypair()
signature = crypto.sign keypair.secret, 'important message'
print! crypto.verify keypair.public, 'important message', signature
check! true
```

## verify

```kototype
|String, String, String| -> Bool
```

Verifies an [Ed25519][ed25519] signature, returning `true` if the signature is
valid for the given message and public key.

The first argument is the 32-byte public key as a hex string, the second is the
message, and the third is the hex-encoded signature.

### Example

```koto
keypair = crypto.keypair()
signature = crypto.sign keypair.secret, 'important message'
print! crypto.verify keypair.public, 'important message', signature
check! true
print! crypto.verify keypair.public, 'tampered message', signature
check! false
```

[blake2]: https://www.blake2.net
[blake3]: https://github.com/BLAKE3-team/BLAKE3
[chacha20poly1305]: https://en.wikipedia.org/wiki/ChaCha20-Poly1305
[ed25519]: https://ed25519.cr.yp.to
[hmac]: https://en.wikipedia.org/wiki/HMAC
[md5]: https://en.wikipedia.org/wiki/MD5
[sha1]: https://en.wikipedia.org/wiki/SHA-1
[sha2]: https://en.wikipedia.org/wiki/SHA-2
