import {
  Account,
  Contract,
  SorobanRpc,
  TransactionBuilder,
  Keypair,
  BASE_FEE,
  StrKey,
  nativeToScVal,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";
import {
  ContractConfig,
  TransactionOptions,
  Ship,
  NebulaLayout,
  ResourceBalance,
  TxResult,
  ShipType,
  ResourceType,
  Signer,
} from "./types";
import { toSigner } from "./signer";

export class StellarNebulaClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private config: ContractConfig;
  /** Placeholder source account used only to simulate read-only contract calls. */
  private readonly simulationKeypair: Keypair;

  constructor(config: ContractConfig) {
    this.config = config;
    this.contract = new Contract(config.contractId);
    this.server = new SorobanRpc.Server(config.rpcUrl);
    this.simulationKeypair = Keypair.random();
  }

  /**
   * Mint a new ship NFT
   */
  async mintShip(
    caller: Keypair | Signer,
    owner: string,
    shipType: ShipType,
    options?: TransactionOptions,
  ): Promise<TxResult<bigint>> {
    return this.executeTransaction(
      caller,
      "mint_ship",
      [owner, shipType],
      options,
    );
  }

  /**
   * Scan a nebula and generate layout
   */
  async scanNebula(
    caller: Keypair | Signer,
    nebulaId: bigint,
    options?: TransactionOptions,
  ): Promise<TxResult<NebulaLayout>> {
    return this.executeTransaction(caller, "scan_nebula", [nebulaId], options);
  }

  /**
   * Harvest resources from a location
   */
  async harvestResources(
    caller: Keypair | Signer,
    shipId: bigint,
    resourceType: ResourceType,
    options?: TransactionOptions,
  ): Promise<TxResult<bigint>> {
    return this.executeTransaction(
      caller,
      "harvest_resources",
      [shipId, resourceType],
      options,
    );
  }

  /**
   * Get ship details by ID
   */
  async getShip(shipId: bigint): Promise<Ship | null> {
    try {
      const result = await this.simulateReadOnlyCall("get_ship", [shipId]);
      return result as Ship;
    } catch (error) {
      return null;
    }
  }

  /**
   * Get resource balance for an address
   */
  async getResourceBalance(
    address: string,
    resourceType: ResourceType,
  ): Promise<bigint> {
    try {
      const result = await this.simulateReadOnlyCall("get_resource_balance", [
        address,
        resourceType,
      ]);
      return BigInt(result as string | number | bigint);
    } catch (error) {
      return BigInt(0);
    }
  }

  /**
   * Converts a plain JS argument into the `xdr.ScVal` the contract call
   * actually requires. `Contract.call()` does not do this conversion
   * itself (it's a thin wrapper over `Operation.invokeContractFunction`),
   * so callers passing native strings/numbers/bigints need it done here.
   * Values that are already `xdr.ScVal` pass through unchanged.
   */
  private toScVal(value: any): xdr.ScVal {
    if (value instanceof xdr.ScVal) {
      return value;
    }
    if (typeof value === "string" && StrKey.isValidEd25519PublicKey(value)) {
      return nativeToScVal(value, { type: "address" });
    }
    // isValidContract exists at runtime in this SDK version but is missing
    // from its published type declarations.
    if (
      typeof value === "string" &&
      (StrKey as unknown as { isValidContract(v: string): boolean }).isValidContract(value)
    ) {
      return nativeToScVal(value, { type: "address" });
    }
    if (typeof value === "bigint") {
      return nativeToScVal(value, { type: "i128" });
    }
    if (typeof value === "number") {
      return nativeToScVal(value, { type: "u32" });
    }
    return nativeToScVal(value);
  }

  private toScValArgs(args: any[]): xdr.ScVal[] {
    return args.map((arg) => this.toScVal(arg));
  }

  /**
   * Simulates a read-only contract invocation (no signature, no submission)
   * and returns its decoded return value. Soroban RPC's simulateTransaction
   * needs a syntactically valid source account to build the envelope, but
   * doesn't require it to exist on-chain for a read-only call, so a
   * throwaway keypair generated for this client instance is used.
   */
  private async simulateReadOnlyCall(
    method: string,
    args: any[],
  ): Promise<any> {
    const account = new Account(this.simulationKeypair.publicKey(), "0");

    const operation = this.contract.call(method, ...this.toScValArgs(args));

    const transaction = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(30)
      .build();

    const simulation = await this.server.simulateTransaction(transaction);

    if (SorobanRpc.Api.isSimulationError(simulation)) {
      throw new Error(simulation.error);
    }
    if (!SorobanRpc.Api.isSimulationSuccess(simulation) || !simulation.result) {
      throw new Error(`Simulation of "${method}" returned no result`);
    }

    return scValToNative(simulation.result.retval);
  }

  /**
   * Stake resources for yield farming
   */
  async stakeResources(
    caller: Keypair | Signer,
    resourceType: ResourceType,
    amount: bigint,
    duration: number,
    options?: TransactionOptions,
  ): Promise<TxResult<void>> {
    return this.executeTransaction(
      caller,
      "stake_resources",
      [resourceType, amount, duration],
      options,
    );
  }

  /**
   * Claim accumulated yield
   */
  async claimYield(
    caller: Keypair | Signer,
    stakeId: bigint,
    options?: TransactionOptions,
  ): Promise<TxResult<bigint>> {
    return this.executeTransaction(caller, "claim_yield", [stakeId], options);
  }

  /**
   * Execute a transaction on the contract
   */
  private async executeTransaction(
    caller: Keypair | Signer,
    method: string,
    args: any[],
    options?: TransactionOptions,
  ): Promise<TxResult> {
    try {
      const signer = toSigner(caller);
      const publicKey = await signer.getPublicKey();
      const account = await this.server.getAccount(publicKey);

      const operation = this.contract.call(method, ...this.toScValArgs(args));

      const transaction = new TransactionBuilder(account, {
        fee: options?.fee || BASE_FEE,
        networkPassphrase: this.config.networkPassphrase,
      })
        .addOperation(operation)
        .setTimeout(options?.timeout || 30)
        .build();

      const signedXdr = await signer.signTransaction(transaction.toXDR(), {
        networkPassphrase: this.config.networkPassphrase,
      });
      const signedTransaction = TransactionBuilder.fromXDR(
        signedXdr,
        this.config.networkPassphrase,
      );

      const response = await this.server.sendTransaction(signedTransaction);

      if (response.status === "PENDING") {
        const txResult = await this.waitForTransaction(response.hash);
        return {
          success: true,
          result: txResult,
          txHash: response.hash,
        };
      }

      return {
        success: false,
        error: "Transaction failed",
      };
    } catch (error: any) {
      return {
        success: false,
        error: error.message || "Unknown error",
      };
    }
  }

  /**
   * Wait for transaction confirmation
   */
  private async waitForTransaction(
    hash: string,
    timeout = 30000,
  ): Promise<any> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const tx = await this.server.getTransaction(hash);
        if (tx.status !== "NOT_FOUND") {
          return tx;
        }
      } catch (error) {
        // Continue polling
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }

    throw new Error("Transaction timeout");
  }
}
