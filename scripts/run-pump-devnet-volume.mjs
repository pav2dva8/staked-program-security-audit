import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from '@solana/web3.js';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';

const RPC_URL = 'https://api.devnet.solana.com';

const WRAPPER_PROGRAM_ID = new PublicKey('3skopQVdqns5x5GjU2c3S4nEcmVbDTkMoZWRaVLsJrAa');
const PUMP_PROGRAM_ID = new PublicKey('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P');
const MAYHEM_PROGRAM_ID = new PublicKey('MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e');
const FEE_PROGRAM_ID = new PublicKey('pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ');
const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

const BUY_EXACT_SOL_IN_DISCRIMINATOR = Buffer.from([56, 252, 116, 8, 158, 223, 205, 95]);
const SELL_DISCRIMINATOR = Buffer.from([51, 230, 133, 164, 1, 127, 131, 173]);

const args = parseArgs(process.argv.slice(2));
const config = {
  mint: new PublicKey(args.mint ?? '5t6j68QiyACSm85tCo7iJgf5zS7RzxR2ZikiBzZduxnL'),
  keypair: args.keypair ?? `${homedir()}/.config/solana/id.json`,
  cycles: Number(args.cycles ?? 8),
  buySol: args.buySol ?? '0.1',
  sellBps: BigInt(args.sellBps ?? 100),
};

if (!Number.isInteger(config.cycles) || config.cycles <= 0 || config.cycles > 25) {
  throw new Error('--cycles must be an integer from 1 to 25');
}
if (config.sellBps < 0n || config.sellBps > 5_000n) {
  throw new Error('--sellBps must be from 0 to 5000');
}

const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(config.keypair, 'utf8'))));
const connection = new Connection(RPC_URL, 'confirmed');
const accounts = derivePumpAccounts(config.mint, payer.publicKey);
const buyLamports = solToLamports(config.buySol);

console.log('Running bounded devnet buy/sell volume');
console.log(`Payer: ${payer.publicKey.toBase58()}`);
console.log(`Mint: ${config.mint.toBase58()}`);
console.log(`Cycles: ${config.cycles}`);
console.log(`Buy per cycle: ${config.buySol} devnet SOL`);
console.log(`Sell per cycle: ${config.sellBps} bps of tokens bought`);
console.log(`Creator vault: ${accounts.creatorVault.toBase58()}`);

await ensureUserAta();
const startBalance = await connection.getBalance(payer.publicKey, 'confirmed');
const startClaimable = await fetchPumpClaimableLamports();

for (let cycle = 1; cycle <= config.cycles; cycle += 1) {
  const before = await tokenAmount(accounts.userTokenAccount);
  const buyTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
    await buildBuyExactSolInInstruction(accounts, payer.publicKey, config.mint, buyLamports),
  );
  const buySig = await sendAndConfirmWithLogs(buyTx, [payer], `buy ${cycle}`);
  const afterBuy = await tokenAmount(accounts.userTokenAccount);
  const bought = afterBuy > before ? afterBuy - before : 0n;
  const sellAmount = (bought * config.sellBps) / 10_000n;

  let sellSig = 'skipped';
  if (sellAmount > 0n) {
    const sellTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
      await buildSellInstruction(accounts, payer.publicKey, config.mint, sellAmount),
    );
    try {
      sellSig = await sendAndConfirmWithLogs(sellTx, [payer], `sell ${cycle}`);
    } catch (error) {
      sellSig = `failed: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  const claimable = await fetchPumpClaimableLamports();
  console.log(JSON.stringify({
    cycle,
    boughtTokens: bought.toString(),
    soldTokens: sellAmount.toString(),
    buySig,
    sellSig,
    claimableLamports: claimable.toString(),
  }));
}

const endBalance = await connection.getBalance(payer.publicKey, 'confirmed');
const endClaimable = await fetchPumpClaimableLamports();
const finalTokenBalance = await connection.getTokenAccountBalance(accounts.userTokenAccount, 'confirmed');

console.log('');
console.log('Done');
console.log(JSON.stringify({
  mint: config.mint.toBase58(),
  payer: payer.publicKey.toBase58(),
  userTokenAccount: accounts.userTokenAccount.toBase58(),
  finalTokenBalanceUi: finalTokenBalance.value.uiAmountString,
  startClaimableLamports: startClaimable.toString(),
  endClaimableLamports: endClaimable.toString(),
  addedClaimableLamports: (endClaimable - startClaimable).toString(),
  startPayerLamports: startBalance,
  endPayerLamports: endBalance,
  spentLamports: startBalance - endBalance,
}, null, 2));

async function ensureUserAta() {
  const info = await connection.getAccountInfo(accounts.userTokenAccount, 'confirmed');
  if (info) return;

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 150_000 }),
    buildCreateAtaIdempotentInstruction(
      payer.publicKey,
      accounts.userTokenAccount,
      payer.publicKey,
      config.mint,
      TOKEN_2022_PROGRAM_ID,
    ),
  );
  await sendAndConfirmWithLogs(tx, [payer], 'create user ATA');
}

async function buildBuyExactSolInInstruction(accountsForMint, user, mintPubkey, spendableSolIn) {
  const globalInfo = await connection.getAccountInfo(accountsForMint.global, 'confirmed');
  if (!globalInfo || globalInfo.data.length < 997) {
    throw new Error('Pump global account was not found or is too small');
  }
  const bondingCurveInfo = await connection.getAccountInfo(accountsForMint.bondingCurve, 'confirmed');
  if (!bondingCurveInfo || bondingCurveInfo.data.length < 81) {
    throw new Error('Pump bonding curve account was not found or is too small');
  }

  const feeRecipient = new PublicKey(globalInfo.data.subarray(41, 73));
  const buybackFeeRecipient = new PublicKey(globalInfo.data.subarray(741, 773));
  assertCurveCreator(accountsForMint, bondingCurveInfo);

  return {
    programId: PUMP_PROGRAM_ID,
    keys: [
      readonly(accountsForMint.global),
      writable(feeRecipient),
      readonly(mintPubkey),
      writable(accountsForMint.bondingCurve),
      writable(accountsForMint.associatedBondingCurve),
      writable(accountsForMint.userTokenAccount),
      writableSigner(user),
      readonly(SystemProgram.programId),
      readonly(TOKEN_2022_PROGRAM_ID),
      writable(accountsForMint.creatorVault),
      readonly(accountsForMint.eventAuthority),
      readonly(PUMP_PROGRAM_ID),
      readonly(accountsForMint.globalVolumeAccumulator),
      writable(accountsForMint.userVolumeAccumulator),
      readonly(accountsForMint.feeConfig),
      readonly(FEE_PROGRAM_ID),
      readonly(accountsForMint.bondingCurveV2),
      writable(buybackFeeRecipient),
    ],
    data: Buffer.concat([
      BUY_EXACT_SOL_IN_DISCRIMINATOR,
      u64(spendableSolIn),
      u64(1n),
      bool(false),
    ]),
  };
}

async function buildSellInstruction(accountsForMint, user, mintPubkey, amount) {
  const globalInfo = await connection.getAccountInfo(accountsForMint.global, 'confirmed');
  if (!globalInfo || globalInfo.data.length < 73) {
    throw new Error('Pump global account was not found or is too small');
  }
  const bondingCurveInfo = await connection.getAccountInfo(accountsForMint.bondingCurve, 'confirmed');
  if (!bondingCurveInfo || bondingCurveInfo.data.length < 81) {
    throw new Error('Pump bonding curve account was not found or is too small');
  }

  const feeRecipient = new PublicKey(globalInfo.data.subarray(41, 73));
  assertCurveCreator(accountsForMint, bondingCurveInfo);

  return {
    programId: PUMP_PROGRAM_ID,
    keys: [
      readonly(accountsForMint.global),
      writable(feeRecipient),
      readonly(mintPubkey),
      writable(accountsForMint.bondingCurve),
      writable(accountsForMint.associatedBondingCurve),
      writable(accountsForMint.userTokenAccount),
      writableSigner(user),
      readonly(SystemProgram.programId),
      writable(accountsForMint.creatorVault),
      readonly(TOKEN_2022_PROGRAM_ID),
      readonly(accountsForMint.eventAuthority),
      readonly(PUMP_PROGRAM_ID),
      readonly(accountsForMint.feeConfig),
      readonly(FEE_PROGRAM_ID),
    ],
    data: Buffer.concat([
      SELL_DISCRIMINATOR,
      u64(amount),
      u64(1n),
    ]),
  };
}

function assertCurveCreator(accountsForMint, bondingCurveInfo) {
  const curveCreator = new PublicKey(bondingCurveInfo.data.subarray(49, 81));
  if (!curveCreator.equals(accountsForMint.feeOwner)) {
    throw new Error(`Bonding curve creator is ${curveCreator.toBase58()}, expected ${accountsForMint.feeOwner.toBase58()}`);
  }
}

async function sendAndConfirmWithLogs(transaction, signers, label) {
  const latestBlockhash = await connection.getLatestBlockhash('confirmed');
  transaction.feePayer = signers[0].publicKey;
  transaction.recentBlockhash = latestBlockhash.blockhash;
  transaction.sign(...signers);

  const simulation = await connection.simulateTransaction(transaction);
  if (simulation.value.err) {
    console.error(`${label} simulation failed:`, JSON.stringify(simulation.value.err));
    console.error((simulation.value.logs ?? []).join('\n'));
    throw new Error(`${label} simulation failed`);
  }

  const signature = await connection.sendRawTransaction(transaction.serialize(), {
    skipPreflight: false,
    preflightCommitment: 'confirmed',
    maxRetries: 5,
  });
  const result = await connection.confirmTransaction({ signature, ...latestBlockhash }, 'confirmed');
  if (result.value.err) {
    throw new Error(`${label} failed: ${JSON.stringify(result.value.err)}`);
  }
  return signature;
}

async function fetchPumpClaimableLamports() {
  const account = await connection.getAccountInfo(accounts.creatorVault, 'confirmed');
  if (!account) return 0n;
  const rentFloor = BigInt(await connection.getMinimumBalanceForRentExemption(account.data.length));
  const lamports = BigInt(account.lamports);
  return lamports > rentFloor ? lamports - rentFloor : 0n;
}

async function tokenAmount(tokenAccount) {
  const balance = await connection.getTokenAccountBalance(tokenAccount, 'confirmed');
  return BigInt(balance.value.amount);
}

function derivePumpAccounts(mintPubkey, user) {
  const feeOwner = findPda(['fee-owner', mintPubkey], WRAPPER_PROGRAM_ID);
  const bondingCurve = findPda(['bonding-curve', mintPubkey], PUMP_PROGRAM_ID);
  return {
    feeOwner,
    mintAuthority: findPda(['mint-authority'], PUMP_PROGRAM_ID),
    bondingCurve,
    bondingCurveV2: findPda(['bonding-curve-v2', mintPubkey], PUMP_PROGRAM_ID),
    associatedBondingCurve: associatedTokenAddress(bondingCurve, TOKEN_2022_PROGRAM_ID, mintPubkey),
    global: findPda(['global'], PUMP_PROGRAM_ID),
    globalParams: findPda(['global-params'], MAYHEM_PROGRAM_ID),
    solVault: findPda(['sol-vault'], MAYHEM_PROGRAM_ID),
    mayhemState: findPda(['mayhem-state', mintPubkey], MAYHEM_PROGRAM_ID),
    mayhemTokenVault: associatedTokenAddress(findPda(['sol-vault'], MAYHEM_PROGRAM_ID), TOKEN_2022_PROGRAM_ID, mintPubkey),
    eventAuthority: findPda(['__event_authority'], PUMP_PROGRAM_ID),
    userTokenAccount: associatedTokenAddress(user, TOKEN_2022_PROGRAM_ID, mintPubkey),
    creatorVault: findPda(['creator-vault', feeOwner], PUMP_PROGRAM_ID),
    globalVolumeAccumulator: findPda(['global_volume_accumulator'], PUMP_PROGRAM_ID),
    userVolumeAccumulator: findPda(['user_volume_accumulator', user], PUMP_PROGRAM_ID),
    feeConfig: findPda(['fee_config', PUMP_PROGRAM_ID], FEE_PROGRAM_ID),
  };
}

function buildCreateAtaIdempotentInstruction(payerPubkey, ata, owner, mintPubkey, tokenProgram) {
  return {
    programId: ASSOCIATED_TOKEN_PROGRAM_ID,
    keys: [
      writableSigner(payerPubkey),
      writable(ata),
      readonly(owner),
      readonly(mintPubkey),
      readonly(SystemProgram.programId),
      readonly(tokenProgram),
    ],
    data: Buffer.from([1]),
  };
}

function findPda(seeds, programId) {
  return PublicKey.findProgramAddressSync(
    seeds.map((seed) => {
      if (typeof seed === 'string') return Buffer.from(seed, 'utf8');
      if (seed instanceof PublicKey) return seed.toBuffer();
      return Buffer.from(seed);
    }),
    programId,
  )[0];
}

function associatedTokenAddress(owner, tokenProgram, mintPubkey) {
  return findPda([owner, tokenProgram, mintPubkey], ASSOCIATED_TOKEN_PROGRAM_ID);
}

function u64(value) {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(BigInt(value));
  return out;
}

function bool(value) {
  return Buffer.from([value ? 1 : 0]);
}

function readonly(pubkey) {
  return { pubkey, isSigner: false, isWritable: false };
}

function writable(pubkey) {
  return { pubkey, isSigner: false, isWritable: true };
}

function writableSigner(pubkey) {
  return { pubkey, isSigner: true, isWritable: true };
}

function solToLamports(value) {
  const [whole, fraction = ''] = String(value).split('.');
  const padded = `${fraction}000000000`.slice(0, 9);
  return BigInt(whole) * 1_000_000_000n + BigInt(padded);
}

function parseArgs(argv) {
  const out = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith('--')) {
      out[key] = 'true';
    } else {
      out[key] = next;
      index += 1;
    }
  }
  return out;
}
