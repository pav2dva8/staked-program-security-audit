import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import { Buffer } from 'buffer';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const RPC_URL = 'https://api.devnet.solana.com';
const WRAPPER_PROGRAM_ID = new PublicKey('3skopQVdqns5x5GjU2c3S4nEcmVbDTkMoZWRaVLsJrAa');
const PUMP_PROGRAM_ID = new PublicKey('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P');
const PUMP_AMM_PROGRAM_ID = new PublicKey('pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA');
const TOKEN_PROGRAM_ID = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const NATIVE_MINT = new PublicKey('So11111111111111111111111111111111111111112');

const DISCRIMINATORS = {
  claimPumpCreatorFees: [44, 231, 59, 108, 53, 79, 88, 215],
  claimPumpSwapCreatorFees: [70, 49, 94, 227, 207, 73, 179, 166],
  increaseStake: [239, 74, 179, 156, 119, 147, 39, 212],
  stake: [206, 176, 202, 18, 200, 209, 179, 108],
};

const args = parseArgs(process.argv.slice(2));
const config = {
  mint: new PublicKey(args.mint ?? '5t6j68QiyACSm85tCo7iJgf5zS7RzxR2ZikiBzZduxnL'),
  keypair: expandHome(args.keypair ?? '~/.config/solana/id.json'),
  walletDir: path.resolve(args.walletDir ?? '.devnet-test-wallets'),
  amounts: parseList(args.amounts ?? '50,75,100'),
  locks: parseList(args.locks ?? '7,30,90').map((value) => Number(value)),
  fundSol: solToLamports(args.fundSol ?? '0.03'),
};

if (config.amounts.length !== config.locks.length) {
  throw new Error('--amounts and --locks must have the same number of entries.');
}

const connection = new Connection(RPC_URL, 'confirmed');
const payer = loadKeypair(config.keypair);
fs.mkdirSync(config.walletDir, { recursive: true });

const mintInfo = await connection.getAccountInfo(config.mint);
if (!mintInfo) throw new Error(`Mint not found: ${config.mint.toBase58()}`);
if (!mintInfo.owner.equals(TOKEN_2022_PROGRAM_ID) && !mintInfo.owner.equals(TOKEN_PROGRAM_ID)) {
  throw new Error(`Mint owner is not an SPL token program: ${mintInfo.owner.toBase58()}`);
}

const tokenProgram = mintInfo.owner;
const decimals = mintInfo.data[44];
const accounts = deriveWrapperAccounts(config.mint, payer.publicKey, tokenProgram);
const launchInfo = await fetchLaunch(accounts.launch);
if (!launchInfo) throw new Error('Launch is not initialized for this mint.');

console.log('Staking from devnet test wallets');
console.log(`Mint: ${config.mint.toBase58()}`);
console.log(`Payer: ${payer.publicKey.toBase58()}`);
console.log(`Wallet dir: ${config.walletDir}`);
console.log(`Token program: ${tokenProgram.toBase58()}`);
console.log(`Decimals: ${decimals}`);

await cleanCreatorFeesIfNeeded(accounts);

const sourceAta = associatedTokenAddress(payer.publicKey, tokenProgram, config.mint);
const sourceBalance = await getTokenAmount(sourceAta);
const totalDeposit = config.amounts.reduce((sum, amount) => sum + parseTokenAmount(amount, decimals), 0n);
if (sourceBalance < totalDeposit) {
  throw new Error(`Payer token balance is too low. Need ${totalDeposit}, have ${sourceBalance}.`);
}

const results = [];
for (let index = 0; index < config.amounts.length; index += 1) {
  const wallet = loadOrCreateWallet(index);
  const amountUi = config.amounts[index];
  const amount = parseTokenAmount(amountUi, decimals);
  const lockDays = config.locks[index];

  await fundWallet(wallet.publicKey);
  const destinationAta = associatedTokenAddress(wallet.publicKey, tokenProgram, config.mint);
  const transferSignature = await transferTokens(sourceAta, destinationAta, wallet.publicKey, amount, decimals, tokenProgram);
  const stakeSignature = await stakeFromWallet(wallet, amount, lockDays, tokenProgram);

  results.push({
    wallet: wallet.publicKey,
    amountUi,
    lockDays,
    transferSignature,
    stakeSignature,
  });
}

const updatedLaunch = await fetchLaunch(accounts.launch);
console.log('\nDone. Test wallets are staked:');
for (const result of results) {
  const walletAccounts = deriveWrapperAccounts(config.mint, result.wallet, tokenProgram);
  const position = await fetchStakePosition(walletAccounts.stakePosition);
  const share = updatedLaunch && position ? formatRewardShare(position.weight, updatedLaunch.totalWeightedStake) : 'unknown';
  console.log(
    [
      `- ${result.wallet.toBase58()}`,
      `${result.amountUi} tokens`,
      `${result.lockDays}d`,
      `share ${share}`,
      `stake tx ${result.stakeSignature}`,
    ].join(' | '),
  );
}

console.log(`\nTotal weighted stake: ${updatedLaunch?.totalWeightedStake.toString() ?? 'unknown'}`);

async function cleanCreatorFeesIfNeeded(payerAccounts) {
  const currentLaunch = await fetchLaunch(payerAccounts.launch);
  if (!currentLaunch || currentLaunch.totalWeightedStake === 0n) return;

  const instructions = [];
  const pumpFeeLamports = await fetchPumpCreatorFeeLamports(payerAccounts.pumpCreatorVault);
  const pumpSwapFeeLamports = await fetchTokenAmountOptional(payerAccounts.pumpSwapCreatorVaultAta);

  if (pumpFeeLamports > 0n) {
    instructions.push(buildClaimPumpCreatorFeesInstruction(payerAccounts, payer.publicKey));
  }

  if (pumpSwapFeeLamports > 0n) {
    const feeOwnerWsolInfo = await connection.getAccountInfo(payerAccounts.feeOwnerWsolAta);
    if (!feeOwnerWsolInfo) {
      instructions.push(
        buildCreateAtaIdempotentInstruction(
          payer.publicKey,
          payerAccounts.feeOwnerWsolAta,
          payerAccounts.feeOwner,
          NATIVE_MINT,
          TOKEN_PROGRAM_ID,
        ),
      );
    }
    instructions.push(buildClaimPumpSwapCreatorFeesInstruction(payerAccounts, payer.publicKey));
  }

  if (!instructions.length) return;

  console.log(`Cleaning creator fees before adding stake: ${pumpFeeLamports} lamports Pump.fun, ${pumpSwapFeeLamports} lamports PumpSwap.`);
  const tx = new Transaction().add(...instructions);
  const signature = await sendAndConfirm(tx, [payer], 'clean creator fees');
  console.log(`Cleaned creator fees: ${signature}`);
}

async function fundWallet(publicKey) {
  const current = await connection.getBalance(publicKey, 'confirmed');
  if (BigInt(current) >= config.fundSol) return;

  const lamports = config.fundSol - BigInt(current);
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: publicKey,
      lamports,
    }),
  );
  const signature = await sendAndConfirm(tx, [payer], `fund ${publicKey.toBase58()}`);
  console.log(`Funded ${publicKey.toBase58()} with ${Number(lamports) / 1_000_000_000} SOL: ${signature}`);
}

async function transferTokens(sourceAta, destinationAta, owner, amount, mintDecimals, tokenProgram) {
  const tx = new Transaction().add(
    buildCreateAtaIdempotentInstruction(payer.publicKey, destinationAta, owner, config.mint, tokenProgram),
    buildTransferCheckedInstruction(sourceAta, destinationAta, payer.publicKey, amount, mintDecimals, tokenProgram),
  );

  const signature = await sendAndConfirm(tx, [payer], `transfer ${amount} tokens to ${owner.toBase58()}`);
  console.log(`Transferred ${formatTokenAmount(amount, mintDecimals)} tokens to ${owner.toBase58()}: ${signature}`);
  return signature;
}

async function stakeFromWallet(wallet, amount, lockDays, tokenProgram) {
  const walletAccounts = deriveWrapperAccounts(config.mint, wallet.publicKey, tokenProgram);
  const hasPosition = Boolean(await connection.getAccountInfo(walletAccounts.stakePosition));
  const discriminator = hasPosition ? DISCRIMINATORS.increaseStake : DISCRIMINATORS.stake;
  const ix = buildStakeInstruction(discriminator, walletAccounts, wallet.publicKey, amount, lockDays, !hasPosition);
  const tx = new Transaction().add(ix);
  const signature = await sendAndConfirm(tx, [wallet], `${hasPosition ? 'increase stake' : 'stake'} ${wallet.publicKey.toBase58()}`);
  console.log(`${hasPosition ? 'Increased stake' : 'Staked'} ${formatTokenAmount(amount, decimals)} for ${wallet.publicKey.toBase58()}: ${signature}`);
  return signature;
}

async function fetchLaunch(launch) {
  const account = await connection.getAccountInfo(launch);
  if (!account) return null;
  return {
    totalWeightedStake: readU128(account.data, 72),
    rewardReserve: account.data.readBigUInt64LE(104),
  };
}

async function fetchStakePosition(stakePosition) {
  const account = await connection.getAccountInfo(stakePosition);
  if (!account) return null;
  return {
    amount: account.data.readBigUInt64LE(8),
    weight: readU128(account.data, 16),
  };
}

async function fetchPumpCreatorFeeLamports(creatorVault) {
  const account = await connection.getAccountInfo(creatorVault);
  if (!account) return 0n;
  const rentFloor = await connection.getMinimumBalanceForRentExemption(account.data.length);
  const lamports = BigInt(account.lamports);
  const rentLamports = BigInt(rentFloor);
  return lamports > rentLamports ? lamports - rentLamports : 0n;
}

async function fetchTokenAmountOptional(tokenAccount) {
  try {
    return await getTokenAmount(tokenAccount);
  } catch {
    return 0n;
  }
}

async function getTokenAmount(tokenAccount) {
  const balance = await connection.getTokenAccountBalance(tokenAccount, 'confirmed');
  return BigInt(balance.value.amount);
}

async function sendAndConfirm(transaction, signers, label) {
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
    maxRetries: 5,
    preflightCommitment: 'confirmed',
    skipPreflight: false,
  });
  const result = await connection.confirmTransaction({ signature, ...latestBlockhash }, 'confirmed');
  if (result.value.err) {
    throw new Error(`${label} failed: ${JSON.stringify(result.value.err)}`);
  }
  return signature;
}

function deriveWrapperAccounts(mint, owner, tokenProgram) {
  const launch = findPda(['launch', mint], WRAPPER_PROGRAM_ID);
  const feeOwner = findPda(['fee-owner', mint], WRAPPER_PROGRAM_ID);
  const protocolFeeVault = findPda(['protocol-fees', mint], WRAPPER_PROGRAM_ID);
  const pumpSwapCreatorVaultAuthority = findPda(['creator_vault', feeOwner], PUMP_AMM_PROGRAM_ID);

  return {
    launch,
    feeOwner,
    protocolFeeVault,
    stakingVault: associatedTokenAddress(launch, tokenProgram, mint),
    userToken: associatedTokenAddress(owner, tokenProgram, mint),
    stakePosition: findPda(['stake', launch, owner], WRAPPER_PROGRAM_ID),
    pumpBondingCurve: findPda(['bonding-curve', mint], PUMP_PROGRAM_ID),
    pumpCreatorVault: findPda(['creator-vault', feeOwner], PUMP_PROGRAM_ID),
    pumpEventAuthority: findPda(['__event_authority'], PUMP_PROGRAM_ID),
    pumpSwapCreatorVaultAuthority,
    pumpSwapCreatorVaultAta: associatedTokenAddress(pumpSwapCreatorVaultAuthority, TOKEN_PROGRAM_ID, NATIVE_MINT),
    feeOwnerWsolAta: associatedTokenAddress(feeOwner, TOKEN_PROGRAM_ID, NATIVE_MINT),
    pumpSwapEventAuthority: findPda(['__event_authority'], PUMP_AMM_PROGRAM_ID),
  };
}

function buildStakeInstruction(discriminator, accounts, staker, amount, lockDays, includeSystemProgram) {
  const data = Buffer.alloc(18);
  data.set(discriminator, 0);
  data.writeBigUInt64LE(amount, 8);
  data.writeUInt16LE(lockDays, 16);

  const keys = [
    writableSigner(staker),
    writable(accounts.launch),
    writable(accounts.userToken),
    readonly(accounts.pumpCreatorVault),
    readonly(accounts.pumpSwapCreatorVaultAta),
    writable(accounts.stakingVault),
    writable(accounts.stakePosition),
    readonly(tokenProgram),
  ];

  if (includeSystemProgram) {
    keys.push(readonly(SystemProgram.programId));
  }

  return new TransactionInstruction({
    programId: WRAPPER_PROGRAM_ID,
    keys,
    data,
  });
}

function buildClaimPumpCreatorFeesInstruction(accounts, signer) {
  return new TransactionInstruction({
    programId: WRAPPER_PROGRAM_ID,
    keys: [
      writable(accounts.launch),
      writableSigner(signer),
      readonly(accounts.pumpBondingCurve),
      writable(accounts.feeOwner),
      writable(accounts.pumpCreatorVault),
      writable(accounts.protocolFeeVault),
      readonly(accounts.pumpEventAuthority),
      readonly(PUMP_PROGRAM_ID),
      readonly(SystemProgram.programId),
    ],
    data: Buffer.from(DISCRIMINATORS.claimPumpCreatorFees),
  });
}

function buildClaimPumpSwapCreatorFeesInstruction(accounts, signer) {
  return new TransactionInstruction({
    programId: WRAPPER_PROGRAM_ID,
    keys: [
      writable(accounts.launch),
      writableSigner(signer),
      writable(accounts.protocolFeeVault),
      readonly(NATIVE_MINT),
      readonly(TOKEN_PROGRAM_ID),
      writable(accounts.feeOwner),
      readonly(accounts.pumpSwapCreatorVaultAuthority),
      writable(accounts.pumpSwapCreatorVaultAta),
      writable(accounts.feeOwnerWsolAta),
      readonly(accounts.pumpSwapEventAuthority),
      readonly(PUMP_AMM_PROGRAM_ID),
      readonly(SystemProgram.programId),
    ],
    data: Buffer.from(DISCRIMINATORS.claimPumpSwapCreatorFees),
  });
}

function buildCreateAtaIdempotentInstruction(payerPubkey, ata, owner, mint, tokenProgram) {
  return new TransactionInstruction({
    programId: ASSOCIATED_TOKEN_PROGRAM_ID,
    keys: [
      writableSigner(payerPubkey),
      writable(ata),
      readonly(owner),
      readonly(mint),
      readonly(SystemProgram.programId),
      readonly(tokenProgram),
    ],
    data: Buffer.from([1]),
  });
}

function buildTransferCheckedInstruction(source, destination, owner, amount, mintDecimals, tokenProgram) {
  const data = Buffer.alloc(10);
  data[0] = 12;
  data.writeBigUInt64LE(amount, 1);
  data[9] = mintDecimals;

  return new TransactionInstruction({
    programId: tokenProgram,
    keys: [
      writable(source),
      readonly(config.mint),
      writable(destination),
      writableSigner(owner),
    ],
    data,
  });
}

function loadOrCreateWallet(index) {
  const filePath = path.join(config.walletDir, `staker-${index + 1}.json`);
  if (fs.existsSync(filePath)) return loadKeypair(filePath);

  const wallet = Keypair.generate();
  fs.writeFileSync(filePath, JSON.stringify(Array.from(wallet.secretKey)));
  return wallet;
}

function loadKeypair(filePath) {
  const secret = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  return Keypair.fromSecretKey(Uint8Array.from(secret));
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

function associatedTokenAddress(owner, tokenProgram, mint) {
  return findPda([owner, tokenProgram, mint], ASSOCIATED_TOKEN_PROGRAM_ID);
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

function parseTokenAmount(value, mintDecimals) {
  const [whole, fraction = ''] = String(value).split('.');
  if (fraction.length > mintDecimals) throw new Error(`${value} exceeds mint decimals ${mintDecimals}.`);
  return BigInt(whole) * 10n ** BigInt(mintDecimals) + BigInt((fraction + '0'.repeat(mintDecimals)).slice(0, mintDecimals));
}

function formatTokenAmount(amount, mintDecimals) {
  const divisor = 10n ** BigInt(mintDecimals);
  const whole = amount / divisor;
  const fraction = (amount % divisor).toString().padStart(mintDecimals, '0').replace(/0+$/, '');
  return `${whole}${fraction ? `.${fraction}` : ''}`;
}

function solToLamports(value) {
  const [whole, fraction = ''] = String(value).split('.');
  return BigInt(whole) * 1_000_000_000n + BigInt((fraction + '0'.repeat(9)).slice(0, 9));
}

function readU128(data, offset) {
  return data.readBigUInt64LE(offset) + (data.readBigUInt64LE(offset + 8) << 64n);
}

function formatRewardShare(weight, totalWeightedStake) {
  if (totalWeightedStake === 0n || weight === 0n) return '0%';
  const hundredths = (weight * 10_000n) / totalWeightedStake;
  if (hundredths === 0n) return '<0.01%';
  const whole = hundredths / 100n;
  const fraction = (hundredths % 100n).toString().padStart(2, '0').replace(/0+$/, '');
  return `${whole}${fraction ? `.${fraction}` : ''}%`;
}

function parseList(value) {
  return String(value)
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
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

function expandHome(filePath) {
  if (filePath === '~') return os.homedir();
  if (filePath.startsWith('~/')) return path.join(os.homedir(), filePath.slice(2));
  return filePath;
}
