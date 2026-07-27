import {
  Address,
  BASE_FEE,
  Contract,
  FeeBumpTransaction,
  SorobanRpc,
  TransactionBuilder,
} from "@stellar/stellar-sdk";
import { EligibilityResult, SPONSOR_ERROR_MESSAGES, SponsorErrorCode } from "./types";

/**
 * Parse a Soroban simulation error string for an embedded contract error
 * code, e.g. `"HostError: Error(Contract, #10)\n\nEvent log..."` -> 10.
 * Returns undefined when the error isn't a recognizable contract error
 * (e.g. a transport/parse failure instead of a contract-level rejection).
 */
export function parseContractErrorCode(error: string): SponsorErrorCode | undefined {
  const match = /Error\(Contract,\s*#(\d+)\)/.exec(error);
  if (!match) return undefined;
  const code = Number(match[1]);
  return code in SponsorErrorCode ? (code as SponsorErrorCode) : undefined;
}

export interface SubmitResult {
  hash: string;
  status: string;
}

/**
 * Thin wrapper around `SorobanRpc.Server` for the two on-chain
 * interactions the relayer needs: a read-only eligibility pre-check
 * (simulated, never submitted) and real fee-bump submission.
 */
export class ContractClient {
  private readonly server: SorobanRpc.Server;

  constructor(
    rpcUrl: string,
    private readonly contractId: string,
    private readonly networkPassphrase: string,
    /**
     * Public key of a funded account used purely as the envelope source
     * for read-only simulations. The sponsor account is used since it is
     * already guaranteed to exist and be funded (it pays fee-bump fees) —
     * no separate throwaway account is needed, and simulateTransaction
     * never commits ledger writes or moves funds regardless.
     */
    private readonly simSourcePublicKey: string,
  ) {
    this.server = new SorobanRpc.Server(rpcUrl);
  }

  /**
   * Calls `gas_sponsor.rs`'s `check_sponsorship_eligibility(player)` view
   * function via simulation, BEFORE the caller builds or submits any real
   * transaction. Requires the contract to expose this function as an
   * invokable entry point (see README's "Known gap" section — this
   * function currently exists in `src/gas_sponsor.rs` but is not yet
   * wired into the contract's public interface in `src/lib.rs`).
   */
  async checkEligibility(playerAddress: string): Promise<EligibilityResult> {
    const account = await this.server.getAccount(this.simSourcePublicKey);
    const contract = new Contract(this.contractId);
    const operation = contract.call(
      "check_sponsorship_eligibility",
      new Address(playerAddress).toScVal(),
    );

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(30)
      .build();

    const sim = await this.server.simulateTransaction(tx);

    if (SorobanRpc.Api.isSimulationError(sim)) {
      const code = parseContractErrorCode(sim.error);
      return {
        eligible: false,
        code,
        message: code !== undefined ? SPONSOR_ERROR_MESSAGES[code] : sim.error,
      };
    }

    return { eligible: true };
  }

  /** Submit a signed fee-bump transaction to the network. */
  async submit(feeBumpTx: FeeBumpTransaction): Promise<SubmitResult> {
    const result = await this.server.sendTransaction(feeBumpTx);

    if (result.status === "ERROR") {
      throw new Error(
        `network rejected the fee-bump transaction (status=ERROR, hash=${result.hash})`,
      );
    }

    return { hash: result.hash, status: result.status };
  }
}
