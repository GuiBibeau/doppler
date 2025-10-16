import {
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  getU64Encoder,
  sendAndConfirmTransactionFactory,
} from "@solana/kit";
import type { Address } from "@solana/addresses";
import { DopplerTransactionBuilder } from "../index.ts";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

type OracleConfig = {
  doppler_program_id: string;
  doppler_admin_key: string;
  oracles: Record<string, string>;
};

const RPC_URL =
  process.env.RPC_URL ?? "https://api.devnet.solana.com";
const WS_URL =
  process.env.WS_URL ??
  RPC_URL.replace("https://", "wss://").replace("http://", "ws://");
const __dirname = dirname(fileURLToPath(import.meta.url));

const ADMIN_KEYPAIR_PATH =
  process.env.ADMIN_KEYPAIR_PATH ??
  resolve(__dirname, "../../deployments/devnet/master-keypair.json");
const DEPLOYMENT_CONFIG_PATH =
  process.env.DEPLOYMENT_CONFIG_PATH ??
  resolve(__dirname, "../../deployments/devnet.json");
const BINANCE_API_URL =
  process.env.BINANCE_API_URL ?? "https://api.binance.com";
const POLL_INTERVAL_MS = Number.parseInt(
  process.env.POLL_INTERVAL_MS ?? "60000",
  10
);
const UNIT_PRICE_MICRO_LAMPORTS = BigInt(
  process.env.UNIT_PRICE_MICRO_LAMPORTS ?? "1000"
);
const PRICE_SCALE = BigInt(process.env.PRICE_SCALE ?? "1000000"); // 1e6

const SYMBOLS = {
  sol_usdc: "SOLUSDT",
  sol_usdt: "SOLUSDT",
  bonk_sol: "BONKUSDT",
} as const;

const sleep = (ms: number) =>
  new Promise((resolve) => setTimeout(resolve, ms));

async function loadKeypair(path: string) {
  const raw = await readFile(path, "utf8");
  const parsed = JSON.parse(raw) as number[];
  return new Uint8Array(parsed);
}

async function loadDeployment(path: string): Promise<OracleConfig> {
  const raw = await readFile(path, "utf8");
  return JSON.parse(raw) as OracleConfig;
}

async function fetchBinancePrice(symbol: string): Promise<number> {
  const url = `${BINANCE_API_URL}/api/v3/ticker/price?symbol=${symbol}`;
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(
      `Binance price request failed (${symbol}): ${res.status} ${res.statusText}`
    );
  }
  const data = (await res.json()) as { price: string };
  const price = Number.parseFloat(data.price);
  if (!Number.isFinite(price)) {
    throw new Error(`Received invalid price for ${symbol}: ${data.price}`);
  }
  return price;
}

function toAtoms(price: number): bigint {
  return BigInt(Math.round(price * Number(PRICE_SCALE)));
}

async function main() {
  const deployment = await loadDeployment(DEPLOYMENT_CONFIG_PATH);
  const keypairBytes = await loadKeypair(ADMIN_KEYPAIR_PATH);
  const signer = await createKeyPairSignerFromBytes(keypairBytes);

  if (signer.address !== deployment.doppler_admin_key) {
    console.warn(
      `Admin key mismatch: deployment config has ${deployment.doppler_admin_key}, loaded keypair is ${signer.address}`
    );
  }

  const rpc = createSolanaRpc(RPC_URL);
  const rpcSubscriptions = createSolanaRpcSubscriptions(WS_URL);
  const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
    rpc,
    rpcSubscriptions,
  });
  const encodePrice = getU64Encoder();

  let running = true;
  const stop = () => {
    running = false;
    console.log("Stopping oracle updater...");
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);

  console.log(
    `Starting Binance oracle updater (interval=${POLL_INTERVAL_MS}ms, rpc=${RPC_URL})`
  );

  while (running) {
    const loopStart = Date.now();
    try {
      const [solUsdt, bonkUsdt] = await Promise.all([
        fetchBinancePrice(SYMBOLS.sol_usdt),
        fetchBinancePrice(SYMBOLS.bonk_sol),
      ]);

      const bonkPerSol = bonkUsdt / solUsdt;

      const updates: Array<{
        oracle: keyof typeof SYMBOLS;
        price: number;
      }> = [
        { oracle: "sol_usdc", price: solUsdt },
        { oracle: "sol_usdt", price: solUsdt },
        { oracle: "bonk_sol", price: bonkPerSol },
      ];

      const builder = new DopplerTransactionBuilder(signer).withUnitPrice(
        UNIT_PRICE_MICRO_LAMPORTS
      );
      const sequence = BigInt(Date.now());

      for (const update of updates) {
        const oracleAddress = deployment.oracles[update.oracle];
        if (!oracleAddress) {
          console.warn(
            `Skipping ${update.oracle} update, oracle address missing in deployment config`
          );
          continue;
        }
        builder.addOracleUpdate({
          oracleAddress: oracleAddress as Address<string>,
          payload: toAtoms(update.price),
          sequence,
          encodePayload: (value) => encodePrice.encode(value),
        });
      }

      const { value: latestBlockhash } = await rpc
        .getLatestBlockhash()
        .send();
      const transaction = await builder.build({
        blockhash: latestBlockhash.blockhash,
        lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
      });

      const signature = await sendAndConfirmTransaction(transaction, {
        commitment: "confirmed",
      });
      console.log(
        `Updated oracles (sig=${signature}) | SOL/USDT=${solUsdt.toFixed(
          4
        )} BONK/SOL=${bonkPerSol.toExponential(6)}`
      );
    } catch (err) {
      console.error("Oracle update failed:", err);
    }

    const elapsed = Date.now() - loopStart;
    const wait = Math.max(0, POLL_INTERVAL_MS - elapsed);
    await sleep(wait);
  }

  process.exit(0);
}

await main();
