# Integration Tests

This directory contains integration tests for moq-ffi that test against real network infrastructure.

## Overview

The integration tests validate end-to-end functionality of the moq-ffi library by connecting to actual MoQ relay servers and performing real publish/subscribe operations.

## Test Suites

### Cloudflare Relay Integration Tests

**File**: `cloudflare_relay_integration.rs`

Tests the library against Cloudflare's production MoQ relay network at:
- **URL**: `https://relay.cloudflare.mediaoverquic.com`
- **Protocol**: IETF Draft 07 (Cloudflare production version)

**Test Cases**:
1. `test_connect_to_cloudflare_relay` - Basic connection establishment
2. `test_connection_lifecycle` - Connect and disconnect flow
3. `test_announce_namespace_requires_connection` - Validation that operations require connection
4. `test_full_publish_workflow` - Complete publish workflow (connect, announce, create publisher, publish data)
5. `test_multiple_clients` - Multiple concurrent client instances
6. `test_error_handling_invalid_url` - Error handling for invalid URLs
7. `test_version_and_utilities` - Utility function validation

## Requirements

### Network Access
- **Internet connectivity** to reach `relay.cloudflare.mediaoverquic.com`
- **Outbound HTTPS** access (port 443)
- **WebTransport over QUIC** support (UDP-based protocol)

### Build Features
Integration tests require the `with_moq_draft07` feature flag since Cloudflare uses IETF Draft 07:

```bash
cargo test --features with_moq_draft07 --test cloudflare_relay_integration
```

## Running the Tests

### Important: Tests are Ignored by Default

All integration tests are marked with `#[ignore]` to prevent them from running automatically during regular CI builds. This is because:
- They require network connectivity
- They depend on external infrastructure availability
- They may be slower than unit tests
- Network issues should not block development builds

### Running Integration Tests

To run the integration tests, use the `--ignored` flag:

```bash
# Run all integration tests
cargo test --features with_moq_draft07 --test cloudflare_relay_integration -- --ignored --nocapture

# Run a specific integration test
cargo test --features with_moq_draft07 --test cloudflare_relay_integration test_connect_to_cloudflare_relay -- --ignored --nocapture

# Run without capturing output (see println! statements)
cargo test --features with_moq_draft07 --test cloudflare_relay_integration -- --ignored --nocapture
```

### Command Breakdown

- `--features with_moq_draft07` - Enables Draft 07 support (required for Cloudflare relay)
- `--test cloudflare_relay_integration` - Specifies which integration test file to run
- `--` - Separates cargo arguments from test arguments
- `--ignored` - Runs tests marked with `#[ignore]`
- `--nocapture` - Shows output from tests (optional, useful for debugging)

## Expected Behavior

### Successful Connection

When tests succeed, you should see output like:

```
=== Test: Connect to Cloudflare Relay ===
Connect result: code=MoqOk
[Callback] Connection state changed to: MoqStateConnecting
[Callback] Connection state changed to: MoqStateConnected
Final connection state: MoqStateConnected
=== Test Complete ===
```

### Connection Timeout or Failure

If the relay is unreachable or network issues occur, tests will handle failures gracefully:

```
=== Test: Connect to Cloudflare Relay ===
Connect result: code=MoqErrorTimeout
Error message: Connection timeout after 30 seconds
Connection did not complete within timeout
=== Test Complete ===
```

This is **not a test failure** - it's expected behavior when network conditions prevent connection.

## Test Design Philosophy

### Network Resilience
- Tests are designed to handle network failures gracefully
- Timeouts are configured (30 seconds for most operations)
- Tests verify correct error handling, not just success paths
- Connection failures are expected and tested

### Real-World Validation
- Tests use actual production infrastructure (Cloudflare relay)
- No mocking - validates real protocol compatibility
- Tests actual WebTransport/QUIC stack
- Confirms TLS certificate validation works

### Production Readiness
These tests fulfill the Production-Readiness recommendations by:
- Testing against real relay infrastructure
- Validating end-to-end workflows
- Confirming error handling
- Verifying multi-client scenarios
- Testing connection lifecycle management

## Debugging Failed Tests

If a test fails:

1. **Check network connectivity**:
   ```bash
   curl -v https://relay.cloudflare.mediaoverquic.com
   ```

2. **Verify DNS resolution**:
   ```bash
   nslookup relay.cloudflare.mediaoverquic.com
   ```

3. **Check firewall rules**:
   - Ensure UDP traffic is allowed (QUIC uses UDP)
   - Verify no proxy blocking WebTransport

4. **Run with verbose output**:
   ```bash
   RUST_LOG=debug cargo test --features with_moq_draft07 --test cloudflare_relay_integration -- --ignored --nocapture
   ```

5. **Test individual cases**:
   ```bash
   cargo test --features with_moq_draft07 test_connection_lifecycle -- --ignored --nocapture
   ```

## CI/CD Integration

### GitHub Actions

To run integration tests in CI/CD, add an optional job:

```yaml
integration-tests:
  name: Integration Tests
  runs-on: ubuntu-latest
  # Only run on manual trigger or specific branches
  if: github.event_name == 'workflow_dispatch' || github.ref == 'refs/heads/integration-test'
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    
    - name: Run Integration Tests
      working-directory: moq_ffi
      run: |
        cargo test --features with_moq_draft07 --test cloudflare_relay_integration -- --ignored --nocapture
      continue-on-error: true  # Don't fail CI on network issues
```

### Best Practices

1. **Don't run on every commit** - Network tests are slow and depend on external services
2. **Use manual triggers** - Run integration tests on-demand or before releases
3. **Allow failures** - External service availability shouldn't block development
4. **Monitor separately** - Track integration test results independently from unit tests

## Contributing

When adding new integration tests:

1. Mark tests with `#[ignore]` if they require network access
2. Add descriptive test names that explain what's being tested
3. Include helpful output with `println!` statements
4. Handle timeouts and network errors gracefully
5. Update this README with new test descriptions

## Future Enhancements

Potential additions to the integration test suite:

- [ ] Subscribe workflow tests (requires coordination with publisher)
- [ ] Large data transfer tests (performance validation)
- [ ] Connection recovery tests (simulate network interruptions)
- [ ] Concurrent operation stress tests
- [ ] Protocol version compatibility tests (Draft 07 vs Draft 14)
- [ ] Mock relay server for offline testing

## References

- [MoQ Transport Draft 07](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/07/)
- [Cloudflare MoQ Relay Documentation](https://developers.cloudflare.com/)
- [Production Readiness Action Plan](../PRODUCTION_READINESS_ACTION_PLAN.md)
