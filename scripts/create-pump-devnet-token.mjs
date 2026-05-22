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
const TOKEN_PROGRAM_ID = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const NATIVE_MINT = new PublicKey('So11111111111111111111111111111111111111112');
const DEVNET_USDC_MINT = new PublicKey('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU');
const ZERO_PUBKEY = new PublicKey('11111111111111111111111111111111');

const CREATE_V2_DISCRIMINATOR = Buffer.from([214, 144, 76, 236, 95, 139, 49, 180]);
const BUY_EXACT_SOL_IN_DISCRIMINATOR = Buffer.from([56, 252, 116, 8, 158, 223, 205, 95]);
const BUY_EXACT_QUOTE_IN_V2_DISCRIMINATOR = Buffer.from([194, 171, 28, 70, 104, 77, 91, 47]);

const DEFAULTS = {
  name: `POB Dev ${new Date().toISOString().slice(11, 16).replace(':', '')}`,
  symbol: 'POBDEV',
  uri: 'https://example.com/pob-devnet-token.json',
  buySol: '0.05',
  buyUsdc: '1',
  keypair: `${homedir()}/.config/solana/id.json`,
};

const args = parseArgs(process.argv.slice(2));
const quote = quoteConfig(args.quote ?? 'sol');
const config = {
  name: args.name ?? DEFAULTS.name,
  symbol: args.symbol ?? DEFAULTS.symbol,
  uri: args.uri ?? DEFAULTS.uri,
  buySol: args.buySol ?? DEFAULTS.buySol,
  buyUsdc: args.buyUsdc ?? args.buyQuote ?? DEFAULTS.buyUsdc,
  keypair: args.keypair ?? DEFAULTS.keypair,
  mint: args.mint,
  quote,
  skipBuy: args.skipBuy === 'true',
};

const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(config.keypair, 'utf8'))));
const mint = config.mint ? null : Keypair.generate();
const mintPubkey = config.mint ? new PublicKey(config.mint) : mint.publicKey;
const connection = new Connection(RPC_URL, 'confirmed');

const accounts = derivePumpAccounts(mintPubkey, payer.publicKey, config.quote);
const buyLamports = solToLamports(config.buySol);
const buyQuoteAmount = decimalToBaseUnits(
  config.quote.symbol === 'USDC' ? config.buyUsdc : config.buySol,
  config.quote.decimals,
);

console.log(`Creating Pump.fun Token-2022 devnet ${config.quote.symbol}-paired token`);
console.log(`Payer: ${payer.publicKey.toBase58()}`);
console.log(`Mint: ${mintPubkey.toBase58()}`);
console.log(`Quote mint: ${config.quote.mint.toBase58()}`);
console.log(`Wrapper fee owner / Pump creator: ${accounts.feeOwner.toBase58()}`);

const balance = await connection.getBalance(payer.publicKey, 'confirmed');
if (balance < 1_000_000_000n) {
  console.log('Wallet has less than 1 SOL on devnet. Requesting airdrop...');
  const airdropSig = await connection.requestAirdrop(payer.publicKey, 2_000_000_000);
  await connection.confirmTransaction(airdropSig, 'confirmed');
}

if (config.quote.symbol === 'USDC' && !config.skipBuy) {
  const usdcBalance = await tokenAmountOptional(accounts.userQuoteAccount);
  if (usdcBalance < buyQuoteAmount) {
    throw new Error([
      `Not enough devnet USDC for the initial buy. Need ${config.buyUsdc} USDC, have ${formatTokenAmount(usdcBalance, config.quote.decimals)} USDC.`,
      `Fund ${payer.publicKey.toBase58()} with Solana Devnet USDC from https://faucet.circle.com/`,
      `Use network "Solana Devnet"; devnet USDC mint is ${DEVNET_USDC_MINT.toBase58()}.`,
      'Then rerun this command.',
    ].join('\n'));
  }
}

let createSig = null;
if (mint) {
  const createTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
    buildCreateV2Instruction(config, accounts, payer.publicKey, mintPubkey),
  );

  createSig = await sendAndConfirmWithLogs(connection, createTx, [payer, mint], 'create_v2');
  console.log(`Create signature: ${createSig}`);
} else {
  console.log('Using existing mint; skipping create_v2.');
}

const prepTx = new Transaction().add(
  ComputeBudgetProgram.setComputeUnitLimit({ units: 150_000 }),
  buildCreateAtaIdempotentInstruction(
    payer.publicKey,
    accounts.userTokenAccount,
    payer.publicKey,
    mintPubkey,
    TOKEN_2022_PROGRAM_ID,
  ),
);
if (config.quote.symbol === 'USDC') {
  prepTx.add(
    buildCreateAtaIdempotentInstruction(
      payer.publicKey,
      accounts.userQuoteAccount,
      payer.publicKey,
      config.quote.mint,
      config.quote.tokenProgram,
    ),
  );
}
const prepSig = await sendAndConfirmWithLogs(connection, prepTx, [payer], 'prepare_user_token_account');
console.log(`Prepare signature: ${prepSig}`);

let buySig = null;
if (!config.skipBuy) {
  const buyInstruction = config.quote.symbol === 'USDC'
    ? await buildBuyExactQuoteInV2Instruction(connection, accounts, payer.publicKey, mintPubkey, config.quote, buyQuoteAmount)
    : await buildBuyExactSolInInstruction(connection, accounts, payer.publicKey, mintPubkey, buyLamports);
  const buyTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 }),
    buyInstruction,
  );

  buySig = await sendAndConfirmWithLogs(
    connection,
    buyTx,
    [payer],
    config.quote.symbol === 'USDC' ? 'buy_exact_quote_in_v2' : 'buy_exact_sol_in',
  );
  console.log(`Buy signature: ${buySig}`);
} else {
  console.log('Skipping initial buy.');
}

const tokenBalance = await connection.getTokenAccountBalance(accounts.userTokenAccount, 'confirmed');
const creatorVaultBalance = config.quote.symbol === 'USDC'
  ? await tokenAmountOptional(accounts.associatedCreatorVault)
  : await connection.getBalance(accounts.creatorVault, 'confirmed').catch(() => 0);

console.log('');
console.log('Done. Paste this mint into the dapp:');
console.log(mintPubkey.toBase58());
console.log('');
console.log(JSON.stringify({
  mint: mintPubkey.toBase58(),
  tokenProgram: TOKEN_2022_PROGRAM_ID.toBase58(),
  quote: config.quote.symbol,
  quoteMint: config.quote.mint.toBase58(),
  payer: payer.publicKey.toBase58(),
  feeOwner: accounts.feeOwner.toBase58(),
  creatorVault: accounts.creatorVault.toBase58(),
  associatedCreatorVault: accounts.associatedCreatorVault.toBase58(),
  userTokenAccount: accounts.userTokenAccount.toBase58(),
  userQuoteAccount: accounts.userQuoteAccount.toBase58(),
  boughtAmountUi: tokenBalance.value.uiAmountString,
  creatorVaultBalance: creatorVaultBalance.toString(),
  createSignature: createSig,
  prepareSignature: prepSig,
  buySignature: buySig,
}, null, 2));

function derivePumpAccounts(mintPubkey, user, quote) {
  const feeOwner = findPda(['fee-owner', mintPubkey], WRAPPER_PROGRAM_ID);
  const mintAuthority = findPda(['mint-authority'], PUMP_PROGRAM_ID);
  const bondingCurve = findPda(['bonding-curve', mintPubkey], PUMP_PROGRAM_ID);
  const bondingCurveV2 = findPda(['bonding-curve-v2', mintPubkey], PUMP_PROGRAM_ID);
  const associatedBondingCurve = associatedTokenAddress(bondingCurve, TOKEN_2022_PROGRAM_ID, mintPubkey);
  const associatedQuoteBondingCurve = associatedTokenAddress(bondingCurve, quote.tokenProgram, quote.mint);
  const global = findPda(['global'], PUMP_PROGRAM_ID);
  const globalParams = findPda(['global-params'], MAYHEM_PROGRAM_ID);
  const solVault = findPda(['sol-vault'], MAYHEM_PROGRAM_ID);
  const mayhemState = findPda(['mayhem-state', mintPubkey], MAYHEM_PROGRAM_ID);
  const mayhemTokenVault = associatedTokenAddress(solVault, TOKEN_2022_PROGRAM_ID, mintPubkey);
  const eventAuthority = findPda(['__event_authority'], PUMP_PROGRAM_ID);
  const userTokenAccount = associatedTokenAddress(user, TOKEN_2022_PROGRAM_ID, mintPubkey);
  const creatorVault = findPda(['creator-vault', feeOwner], PUMP_PROGRAM_ID);
  const associatedCreatorVault = associatedTokenAddress(creatorVault, quote.tokenProgram, quote.mint);
  const sharingConfig = findPda(['sharing-config', mintPubkey], FEE_PROGRAM_ID);
  const globalVolumeAccumulator = findPda(['global_volume_accumulator'], PUMP_PROGRAM_ID);
  const userVolumeAccumulator = findPda(['user_volume_accumulator', user], PUMP_PROGRAM_ID);
  const associatedUserVolumeAccumulator = associatedTokenAddress(userVolumeAccumulator, quote.tokenProgram, quote.mint);
  const feeConfig = findPda(['fee_config', PUMP_PROGRAM_ID], FEE_PROGRAM_ID);
  const userQuoteAccount = associatedTokenAddress(user, quote.tokenProgram, quote.mint);

  return {
    feeOwner,
    mintAuthority,
    bondingCurve,
    bondingCurveV2,
    associatedBondingCurve,
    associatedQuoteBondingCurve,
    global,
    globalParams,
    solVault,
    mayhemState,
    mayhemTokenVault,
    eventAuthority,
    userTokenAccount,
    userQuoteAccount,
    creatorVault,
    associatedCreatorVault,
    sharingConfig,
    globalVolumeAccumulator,
    userVolumeAccumulator,
    associatedUserVolumeAccumulator,
    feeConfig,
  };
}

function buildCreateV2Instruction(config, accounts, payerPubkey, mintPubkey) {
  const keys = [
    writableSigner(mintPubkey),
    readonly(accounts.mintAuthority),
    writable(accounts.bondingCurve),
    writable(accounts.associatedBondingCurve),
    readonly(accounts.global),
    writableSigner(payerPubkey),
    readonly(SystemProgram.programId),
    readonly(TOKEN_2022_PROGRAM_ID),
    readonly(ASSOCIATED_TOKEN_PROGRAM_ID),
    writable(MAYHEM_PROGRAM_ID),
    readonly(accounts.globalParams),
    writable(accounts.solVault),
    writable(accounts.mayhemState),
    writable(accounts.mayhemTokenVault),
    readonly(accounts.eventAuthority),
    readonly(PUMP_PROGRAM_ID),
  ];

  if (config.quote.symbol !== 'SOL') {
    keys.push(
      readonly(config.quote.mint),
      writable(accounts.associatedQuoteBondingCurve),
      readonly(config.quote.tokenProgram),
    );
  }

  return {
    programId: PUMP_PROGRAM_ID,
    keys,
    data: Buffer.concat([
      CREATE_V2_DISCRIMINATOR,
      borshString(config.name),
      borshString(config.symbol),
      borshString(config.uri),
      accounts.feeOwner.toBuffer(),
      bool(false),
      bool(false),
    ]),
  };
}

async function buildBuyExactQuoteInV2Instruction(connection, accounts, user, mintPubkey, quote, spendableQuoteIn) {
  const globalInfo = await connection.getAccountInfo(accounts.global, 'confirmed');
  if (!globalInfo || globalInfo.data.length < 997) {
    throw new Error('Pump global account was not found or is too small');
  }
  const bondingCurveInfo = await connection.getAccountInfo(accounts.bondingCurve, 'confirmed');
  if (!bondingCurveInfo || bondingCurveInfo.data.length < 115) {
    throw new Error('Pump bonding curve account was not found or is too small');
  }

  const global = decodePumpGlobal(globalInfo.data);
  const curveCreator = new PublicKey(bondingCurveInfo.data.subarray(49, 81));
  const curveQuoteMint = new PublicKey(bondingCurveInfo.data.subarray(83, 115));

  if (!curveCreator.equals(accounts.feeOwner)) {
    throw new Error(`Bonding curve creator is ${curveCreator.toBase58()}, expected ${accounts.feeOwner.toBase58()}`);
  }
  if (!curveQuoteMint.equals(quote.mint)) {
    throw new Error(`Bonding curve quote mint is ${curveQuoteMint.toBase58()}, expected ${quote.mint.toBase58()}`);
  }

  return {
    programId: PUMP_PROGRAM_ID,
    keys: [
      readonly(accounts.global),
      readonly(mintPubkey),
      readonly(quote.mint),
      readonly(TOKEN_2022_PROGRAM_ID),
      readonly(quote.tokenProgram),
      readonly(ASSOCIATED_TOKEN_PROGRAM_ID),
      writable(global.feeRecipient),
      writable(associatedTokenAddress(global.feeRecipient, quote.tokenProgram, quote.mint)),
      writable(global.buybackFeeRecipient),
      writable(associatedTokenAddress(global.buybackFeeRecipient, quote.tokenProgram, quote.mint)),
      writable(accounts.bondingCurve),
      writable(accounts.associatedBondingCurve),
      writable(accounts.associatedQuoteBondingCurve),
      writableSigner(user),
      writable(accounts.userTokenAccount),
      writable(accounts.userQuoteAccount),
      writable(accounts.creatorVault),
      writable(accounts.associatedCreatorVault),
      readonly(accounts.sharingConfig),
      readonly(accounts.globalVolumeAccumulator),
      writable(accounts.userVolumeAccumulator),
      writable(accounts.associatedUserVolumeAccumulator),
      readonly(accounts.feeConfig),
      readonly(FEE_PROGRAM_ID),
      readonly(SystemProgram.programId),
      readonly(accounts.eventAuthority),
      readonly(PUMP_PROGRAM_ID),
    ],
    data: Buffer.concat([
      BUY_EXACT_QUOTE_IN_V2_DISCRIMINATOR,
      u64(spendableQuoteIn),
      u64(1n),
    ]),
  };
}

async function buildBuyExactSolInInstruction(connection, accounts, user, mintPubkey, spendableSolIn) {
  const globalInfo = await connection.getAccountInfo(accounts.global, 'confirmed');
  if (!globalInfo || globalInfo.data.length < 997) {
    throw new Error('Pump global account was not found or is too small');
  }
  const bondingCurveInfo = await connection.getAccountInfo(accounts.bondingCurve, 'confirmed');
  if (!bondingCurveInfo || bondingCurveInfo.data.length < 81) {
    throw new Error('Pump bonding curve account was not found or is too small');
  }

  const feeRecipient = new PublicKey(globalInfo.data.subarray(41, 73));
  const buybackFeeRecipient = new PublicKey(globalInfo.data.subarray(741, 773));
  const curveCreator = new PublicKey(bondingCurveInfo.data.subarray(49, 81));

  if (!curveCreator.equals(accounts.feeOwner)) {
    throw new Error(`Bonding curve creator is ${curveCreator.toBase58()}, expected ${accounts.feeOwner.toBase58()}`);
  }

  return {
    programId: PUMP_PROGRAM_ID,
    keys: [
      readonly(accounts.global),
      writable(feeRecipient),
      readonly(mintPubkey),
      writable(accounts.bondingCurve),
      writable(accounts.associatedBondingCurve),
      writable(accounts.userTokenAccount),
      writableSigner(user),
      readonly(SystemProgram.programId),
      readonly(TOKEN_2022_PROGRAM_ID),
      writable(accounts.creatorVault),
      readonly(accounts.eventAuthority),
      readonly(PUMP_PROGRAM_ID),
      readonly(accounts.globalVolumeAccumulator),
      writable(accounts.userVolumeAccumulator),
      readonly(accounts.feeConfig),
      readonly(FEE_PROGRAM_ID),
      readonly(accounts.bondingCurveV2),
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

async function sendAndConfirmWithLogs(connection, transaction, signers, label) {
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

function borshString(value) {
  const bytes = Buffer.from(value, 'utf8');
  const length = Buffer.alloc(4);
  length.writeUInt32LE(bytes.length);
  return Buffer.concat([length, bytes]);
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
  return decimalToBaseUnits(value, 9);
}

function decimalToBaseUnits(value, decimals) {
  const [whole, fraction = ''] = String(value).split('.');
  const padded = `${fraction}${'0'.repeat(decimals)}`.slice(0, decimals);
  return BigInt(whole) * (10n ** BigInt(decimals)) + BigInt(padded || '0');
}

function formatTokenAmount(value, decimals) {
  const divisor = 10n ** BigInt(decimals);
  const whole = value / divisor;
  const fraction = (value % divisor).toString().padStart(decimals, '0').replace(/0+$/, '');
  return `${whole}${fraction ? `.${fraction}` : ''}`;
}

async function tokenAmountOptional(tokenAccount) {
  try {
    const balance = await connection.getTokenAccountBalance(tokenAccount, 'confirmed');
    return BigInt(balance.value.amount);
  } catch {
    return 0n;
  }
}

function quoteConfig(value) {
  const normalized = String(value).toLowerCase();
  if (normalized === 'sol' || normalized === 'wsol') {
    return {
      symbol: 'SOL',
      mint: NATIVE_MINT,
      tokenProgram: TOKEN_PROGRAM_ID,
      decimals: 9,
    };
  }
  if (normalized === 'usdc') {
    return {
      symbol: 'USDC',
      mint: DEVNET_USDC_MINT,
      tokenProgram: TOKEN_PROGRAM_ID,
      decimals: 6,
    };
  }
  throw new Error('--quote must be sol or usdc.');
}

function decodePumpGlobal(data) {
  const feeRecipient = new PublicKey(data.subarray(41, 73));
  const buybackFeeRecipient = new PublicKey(data.subarray(741, 773));
  const whitelistedQuoteMint = data.length >= 1013
    ? new PublicKey(data.subarray(1013, 1045))
    : ZERO_PUBKEY;

  return {
    feeRecipient,
    buybackFeeRecipient,
    whitelistedQuoteMint,
  };
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
