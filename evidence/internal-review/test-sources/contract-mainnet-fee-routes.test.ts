// Offline execution of the real Evolut SBF and hash-pinned mainnet Raydium SBF.
// Config #13 and permission are synthetic VM fixtures, NOT live authorization.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { LiteSVM, FailedTransactionMetadata, ComputeBudget } from 'litesvm';
import { PublicKey, Keypair, Transaction, TransactionInstruction, SystemProgram, ComputeBudgetProgram, SYSVAR_CLOCK_PUBKEY,
  AddressLookupTableAccount, AddressLookupTableProgram, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, NATIVE_MINT,
  getAssociatedTokenAddressSync, createAssociatedTokenAccountIdempotentInstruction, createSyncNativeInstruction,
  createTransferInstruction } from '@solana/spl-token';
import { curveIx, decodeAccount, curveProgram as program } from './curve-idl-codec.ts';
import { contractMainnetBinaryUrl, contractMainnetProgramId } from '../contract-artifacts.mjs';
import { verifyFixtureSet } from '../../../operations-rollout/server/vm-tests/verify-fixtures.mjs';
import { buildGraduationInstruction, graduationLookupAddresses } from '../../../operations-rollout/server/src/services/raydium-graduation.js';
import { buildRaydiumCreatorFeeClaim } from '../../../operations-rollout/server/src/services/raydium-creator-fees.js';
import { decodeRaydiumPool, raydiumReserves } from '../../../operations-rollout/server/src/chain/raydium-cpmm.js';
import { decodeRaydiumTradingFees } from '../../../operations-rollout/server/src/services/raydium-pool.service.js';
import { quoteStaticSwap } from '../../../operations-rollout/lib/trade-core/quote.js';
import { mapTradeSnapshot } from '../../../operations-rollout/server/src/services/trade-context.service.js';
import { hydrateSwapQuote } from '../../../operations-rollout/lib/trade-core/client-context.js';

assert.equal(program.toBase58(), contractMainnetProgramId, 'run with EVOLUT_VM_NETWORK=mainnet');
const root = new URL('../../../operations-rollout/server/vm-tests/fixtures/mainnet-capture-V2rzRm/', import.meta.url);
const manifest = JSON.parse(readFileSync(new URL('../../../operations-rollout/server/vm-tests/external-fixtures.mainnet.json', import.meta.url), 'utf8'));
verifyFixtureSet(root, manifest.files);
const captured = JSON.parse(readFileSync(new URL('raydium/accounts.json', root), 'utf8'));
const candidate = JSON.parse(readFileSync(new URL('../mainnet-candidate.json', import.meta.url), 'utf8'));
const legacyBinary = new URL('../../../operations-rollout/contracts/evolut-curve/target/deploy-mainnet/dynamic_bonding_curve.so', import.meta.url);
assert.equal(createHash('sha256').update(readFileSync(legacyBinary)).digest('hex'),'2fce058ef05c9b3e1ef70f4553b94c0733e51988db9a6a0769c68406b48d5615');
const treasury = new PublicKey('ASemhYRLkceP777SAfYVShjdjt481bKw7xUJuaj8u7a4');
const admin = new PublicKey('5HF4Vno1pAdY8UdPMqrpAGtziNUX6kFTCH1qMokDPsBP');
const cpmm = new PublicKey('CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C');
const locker = new PublicKey('LockrWmn6K5twhz3y9w1dQERbmgSaRkfnTeTKbpofwE');
const metadata = new PublicKey('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');
const pda = (id: PublicKey, ...seeds: (string | Buffer)[]) => PublicKey.findProgramAddressSync(seeds.map(s => typeof s === 'string' ? Buffer.from(s) : s), id)[0];
const disc = (name: string) => createHash('sha256').update(name).digest().subarray(0, 8);
const authority = pda(program, 'pool_authority'), cpAuthority = pda(cpmm, 'vault_and_lp_mint_auth_seed');
const amount = (svm: LiteSVM, key: PublicKey) => { const a = svm.getAccount(key); return a ? Buffer.from(a.data).readBigUInt64LE(64) : 0n; };

function context(tier: 13 | 2 = 13, production = false, baseFirst = true, legacy = false) {
  const svm = new LiteSVM(), budget = new ComputeBudget();
  budget.computeUnitLimit = 1_400_000n; budget.maxInstructionStackDepth = 5n; budget.maxInstructionTraceLength = 64n;
  svm.withComputeBudget(budget).withLogBytesLimit(100_000n);
  for (const [id, path] of [[program, contractMainnetBinaryUrl], [cpmm, new URL('raydium/cpmm.so', root)],
    [locker, new URL('raydium/lock.so', root)], [metadata, new URL('metadata.so', root)]] as const)
    svm.addProgramFromFile(id, fileURLToPath(id.equals(program) && legacy ? legacyBinary : path));
  for (const a of captured) svm.setAccount(new PublicKey(a.address), { ...a, owner: new PublicKey(a.owner), data: Buffer.from(a.data, 'base64'), rentEpoch: 0 });
  const clock = svm.getClock(); clock.unixTimestamp = 1_800_000_000n; clock.slot = 1000n; svm.setClock(clock);
  const configSeed = Buffer.alloc(2); configSeed.writeUInt16BE(tier);
  const [rayConfig, configBump] = PublicKey.findProgramAddressSync([Buffer.from('amm_config'), configSeed], cpmm);
  const data = Buffer.from(captured[1].data, 'base64'); data[8] = configBump; data.writeUInt16LE(tier, 10);
  data.writeBigUInt64LE(tier === 13 ? 300n : 20_000n, 12); data.writeBigUInt64LE(tier === 13 ? 9700n : 500n, 108);
  // Explicit fixture: 5% protocol share of creator fees, not a current live quote.
  data.writeBigUInt64LE(50_000n, 116);
  svm.setAccount(rayConfig, { owner: cpmm, data, lamports: 3_000_000, executable: false, rentEpoch: 0 });
  const permission = pda(cpmm, 'permission', authority.toBuffer());
  const permissionData = Buffer.alloc(280); disc('account:Permission').copy(permissionData); authority.toBuffer().copy(permissionData, 8);
  svm.setAccount(permission, { owner: cpmm, data: permissionData, lamports: 4_000_000, executable: false, rentEpoch: 0 });
  const payer = Keypair.generate(), creator = Keypair.generate(), config = Keypair.generate();
  let mint = Keypair.generate();
  while ((Buffer.compare(mint.publicKey.toBuffer(), NATIVE_MINT.toBuffer()) < 0) !== baseFirst) mint = Keypair.generate();
  for (const key of [payer.publicKey, creator.publicKey, treasury, admin, authority]) svm.airdrop(key, 500_000_000_000n);
  function send(ixs: TransactionInstruction[], signers = [payer], fail = false, fixtureSigner?: PublicKey) {
    svm.expireBlockhash();
    const tx = new Transaction({ feePayer: signers[0].publicKey, recentBlockhash: svm.latestBlockhash() })
      .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }), ...ixs);
    tx.partialSign(...signers); if (fixtureSigner) tx.addSignature(fixtureSigner, Buffer.alloc(64));
    assert.ok(tx.serialize({ requireAllSignatures: true, verifySignatures: !fixtureSigner }).length <= 1232);
    // Only explicit setup/treasury tests substitute a known public identity's
    // signature. User/attacker claims and graduation use real test signatures.
    svm.withSigverify(!fixtureSigner); const r = svm.sendTransaction(tx); svm.withSigverify(true);
    if (fail) assert.ok(r instanceof FailedTransactionMetadata, 'must reject');
    else if (r instanceof FailedTransactionMetadata) assert.fail(String(r.err()) + '\n' + r.meta().logs().slice(-30).join('\n'));
    return r;
  }
  const study = candidate.offlineCurveStudy[production ? 'production' : 'test'];
  const params = { pool_fees: { base_fee: { cliff_fee_numerator: tier === 2 && !legacy ? 20_000_000n : 10_000_000n }, dynamic_fee: null }, collect_fee_mode: 0,
    activation_type: 0, token_decimal: 6, migration_quote_threshold: BigInt(study.thresholdLamports),
    sqrt_start_price: BigInt(study.sqrtStartPrice), locked_vesting: {},
    token_supply: { pre_migration_token_supply: 1_000_000_000_000_000n, post_migration_token_supply: 0n },
    token_update_authority: 0, pool_creation_fee: 0n, max_setup_lamports: BigInt(candidate.confirmedLaunchEconomics.graduationSetupCostCapLamports),
    curve: [{ sqrt_price: BigInt(study.bandEndSqrtPrice), liquidity: BigInt(study.liquidity) }] };
  const common = { pool_authority: authority, token_program: TOKEN_PROGRAM_ID, token_base_program: TOKEN_PROGRAM_ID,
    token_quote_program: TOKEN_PROGRAM_ID, system_program: SystemProgram.programId,
    event_authority: pda(program, '__event_authority'), program, metadata_program: metadata };
  send([curveIx('create_config', { ...common, config: config.publicKey, fee_claimer: treasury, leftover_receiver: treasury,
    quote_mint: NATIVE_MINT, raydium_amm_config: rayConfig, payer: payer.publicKey }, { config_parameters: params })], [payer, config], false, treasury);
  if (legacy) svm.addProgramFromFile(program, fileURLToPath(contractMainnetBinaryUrl));
  const order = [mint.publicKey, NATIVE_MINT].sort((a, b) => Buffer.compare(a.toBuffer(), b.toBuffer()));
  const pool = pda(program, 'pool', config.publicKey.toBuffer(), order[1].toBuffer(), order[0].toBuffer());
  const baseVault = pda(program, 'token_vault', mint.publicKey.toBuffer(), pool.toBuffer());
  const quoteVault = pda(program, 'token_vault', NATIVE_MINT.toBuffer(), pool.toBuffer());
  const baseAta = getAssociatedTokenAddressSync(mint.publicKey, creator.publicKey), quoteAta = getAssociatedTokenAddressSync(NATIVE_MINT, creator.publicKey);
  send([curveIx('initialize_virtual_pool_with_spl_token', { ...common, config: config.publicKey, creator: creator.publicKey,
    base_mint: mint.publicKey, quote_mint: NATIVE_MINT, pool, base_vault: baseVault, quote_vault: quoteVault,
    mint_metadata: pda(metadata, 'metadata', metadata.toBuffer(), mint.publicKey.toBuffer()), payer: payer.publicKey },
    { params: { name: 'Mainnet fixture', symbol: 'VM', uri: 'https://example.com/fixture' } })], [payer, creator, mint]);
  const quoteResult: any = send([curveIx('quote_paid_launch_buy', { config: config.publicKey }, { amount: 100_000_000_000n })]);
  const spend = Buffer.from(quoteResult.returnData().data()).readBigUInt64LE(0);
  send([createAssociatedTokenAccountIdempotentInstruction(creator.publicKey, baseAta, creator.publicKey, mint.publicKey),
    createAssociatedTokenAccountIdempotentInstruction(creator.publicKey, quoteAta, creator.publicKey, NATIVE_MINT),
    SystemProgram.transfer({ fromPubkey: creator.publicKey, toPubkey: quoteAta, lamports: spend }), createSyncNativeInstruction(quoteAta),
    curveIx('swap2', { ...common, config: config.publicKey, pool, base_mint: mint.publicKey, quote_mint: NATIVE_MINT,
      base_vault: baseVault, quote_vault: quoteVault, input_token_account: quoteAta, output_token_account: baseAta, payer: creator.publicKey },
      { params: { amount_0: spend, amount_1: 1n, swap_mode: 1 } })], [creator]);
  const readPool = () => decodeAccount('VirtualPool', svm.getAccount(pool)!.data);
  assert.equal(readPool().migration_progress, 2);
  const curveNumerator = tier === 2 && !legacy ? 20_000_000n : 10_000_000n;
  assert.equal(Buffer.from(svm.getAccount(config.publicKey)!.data).readBigUInt64LE(136), curveNumerator);
  const chargedCurveFee = spend - readPool().quote_reserve;
  assert.equal(chargedCurveFee, (spend * curveNumerator + 999_999_999n) / 1_000_000_000n);
  assert.equal(readPool().metrics.total_trading_quote_fee, chargedCurveFee);
  assert.equal(readPool().metrics.total_protocol_quote_fee, 0n);
  assert.equal(readPool().creator_quote_fee, chargedCurveFee / 2n);
  assert.equal(readPool().partner_quote_fee, chargedCurveFee - chargedCurveFee / 2n);
  const reimbursement = getAssociatedTokenAddressSync(NATIVE_MINT, payer.publicKey);
  send([createAssociatedTokenAccountIdempotentInstruction(payer.publicKey, reimbursement, payer.publicKey, NATIVE_MINT)]);
  const source = { poolAddress: pool.toBase58(), configAddress: config.publicKey.toBase58(), address: mint.publicKey.toBase58(),
    baseVault: baseVault.toBase58(), quoteVault: quoteVault.toBase58() };
  const g = buildGraduationInstruction({ program, network: 'mainnet', source, raydiumConfig: rayConfig, creator: creator.publicKey,
    partner: treasury, payer: payer.publicKey, nonce: Buffer.alloc(32) });
  function graduate(fail = false) {
    // Use the real shared-table contents, not a per-pool table that hides size problems.
    const lookup = graduationLookupAddresses(program,'mainnet',payer.publicKey,[rayConfig]);
    const key = Keypair.generate().publicKey, bytes = Buffer.alloc(56 + 32 * lookup.length);
    bytes.writeUInt32LE(1); bytes.writeBigUInt64LE(0xffffffffffffffffn, 4);
    lookup.forEach((p, i) => p.toBuffer().copy(bytes, 56 + i * 32));
    svm.setAccount(key, { owner: AddressLookupTableProgram.programId, data: bytes, lamports: 100_000_000, executable: false, rentEpoch: 0 });
    const table = new AddressLookupTableAccount({ key, state: { deactivationSlot: 0xffffffffffffffffn,
      lastExtendedSlot: 0, lastExtendedSlotStartIndex: 0, authority: undefined, addresses: lookup } });
    svm.expireBlockhash();
    const tx = new VersionedTransaction(new TransactionMessage({ payerKey: payer.publicKey, recentBlockhash: svm.latestBlockhash(),
      instructions: [ComputeBudgetProgram.setComputeUnitLimit({ units: 650_000 }), g.instruction] }).compileToV0Message([table]));
    tx.sign([payer]); assert.ok(tx.serialize().length <= 1232);
    const r = svm.sendTransaction(tx);
    if (fail) assert.ok(r instanceof FailedTransactionMetadata);
    else if (r instanceof FailedTransactionMetadata) assert.fail(String(r.err()) + '\n' + r.meta().logs().slice(-35).join('\n'));
    return r;
  }
  function trade(buy = true) {
    const userBase = getAssociatedTokenAddressSync(mint.publicKey, payer.publicKey);
    send([createAssociatedTokenAccountIdempotentInstruction(payer.publicKey, userBase, payer.publicKey, mint.publicKey),
      SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: reimbursement, lamports: 100_000_000n }), createSyncNativeInstruction(reimbursement)]);
    const clock = svm.getClock(); clock.unixTimestamp += 2n; svm.setClock(clock);
    const inputMint = buy ? NATIVE_MINT : mint.publicKey, outputMint = buy ? mint.publicKey : NATIVE_MINT;
    const keys = [payer.publicKey, cpAuthority, rayConfig, g.addresses.pool, buy ? reimbursement : userBase, buy ? userBase : reimbursement,
      g.addresses.vault(inputMint), g.addresses.vault(outputMint), TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID, inputMint, outputMint, g.addresses.observation];
    const data = Buffer.alloc(24); disc('global:swap_base_input').copy(data);
    data.writeBigUInt64LE(buy ? 10_000_000n : amount(svm, userBase) / 2n, 8); data.writeBigUInt64LE(1n, 16);
    const info=(key:PublicKey)=>{const a=svm.getAccount(key)!;return {...a,data:Buffer.from(a.data),rentEpoch:0};};
    const state=decodeRaydiumPool(info(g.addresses.pool),g.addresses.pool,'mainnet',{configuration:rayConfig,baseMint:mint.publicKey});
    const reserves=raydiumReserves(state,{address:g.addresses.vault(order[0]),account:info(g.addresses.vault(order[0]))},
      {address:g.addresses.vault(order[1]),account:info(g.addresses.vault(order[1]))});
    const fees=decodeRaydiumTradingFees(info(rayConfig),rayConfig,'mainnet');
    const quote=quoteStaticSwap({kind:'raydium-cpmm',reserveA:state.baseIsToken0?reserves.baseRaw:reserves.quoteRaw,
      reserveB:state.baseIsToken0?reserves.quoteRaw:reserves.baseRaw,enabled:state.swapEnabled,openTime:state.openTime,
      currentTime:clock.unixTimestamp,fee:{...fees,creatorFeeEnabled:!!state.creatorFeeEnabled,creatorFeeOn:state.creatorFeeOn}},
      {amountIn:data.readBigUInt64LE(8),aToB:inputMint.equals(order[0]),mode:'exact-in',slippageBps:0});
    const rawKeys=[g.addresses.pool,rayConfig,...order,...order.map(m=>g.addresses.vault(m)),SYSVAR_CLOCK_PUBKEY];
    const mapped=mapTradeSnapshot({pool:g.addresses.pool.toBase58(),program:cpmm.toBase58(),kind:'raydium_cpmm'},
      {slot:Number(clock.slot),accounts:rawKeys.map(key=>{const a=info(key);return {address:key.toBase58(),owner:a.owner.toBase58(),data:a.data,executable:a.executable};})},
      {enabled:true,network:'mainnet-beta',dbcProgram:program.toBase58(),raydiumProgram:cpmm.toBase58(),dbcConfigs:[],
        computeUnits:400_000,microLamportsPerComputeUnit:1000n,maxPriorityFeeLamports:1000n,signatureFeeLamports:5000n});
    const browserQuote=quoteStaticSwap(hydrateSwapQuote(mapped as any),
      {amountIn:data.readBigUInt64LE(8),aToB:inputMint.equals(order[0]),mode:'exact-in',slippageBps:0});
    assert.deepEqual(browserQuote,quote,'API account mapper and browser hydration preserve exact quote');
    const destination=buy?userBase:reimbursement,before=amount(svm,destination);
    send([new TransactionInstruction({ programId: cpmm, data, keys: keys.map((pubkey, i) => ({ pubkey, isSigner: i === 0, isWritable: [3,4,5,6,7,12].includes(i) })) })]);
    assert.equal(amount(svm,destination)-before,quote.output,'shared frontend quote exactly matches Raydium SBF output');
  }
  const feeAuthority = pda(program, 'ray_creator_fees', pool.toBuffer());
  const collected = order.map(m => getAssociatedTokenAddressSync(m, feeAuthority, true));
  const ledger = pda(program, 'ray_creator_ledger', pool.toBuffer());
  const share = pda(cpmm, 'creator_fee_share', feeAuthority.toBuffer(), rayConfig.toBuffer());
  function claim(side: number, owner = side === 0 ? creator.publicKey : treasury, overrides = {}) {
    const nft = g.addresses.positions[side === 1 ? 1 : 0];
    const ix = curveIx('claim_raydium_creator_fees', { owner, virtual_pool: pool, config: config.publicKey,
      fee_authority: feeAuthority, ledger, fee_nft_account: getAssociatedTokenAddressSync(nft.nft, owner), fee_lock: nft.lock,
      raydium_pool: g.addresses.pool, raydium_amm_config: rayConfig, cpmm_program: cpmm, cpmm_authority: cpAuthority,
      token_0_mint: order[0], token_1_mint: order[1], token_0_vault: g.addresses.vault(order[0]), token_1_vault: g.addresses.vault(order[1]),
      collected_0: collected[0], collected_1: collected[1], destination_0: getAssociatedTokenAddressSync(order[0], owner),
      destination_1: getAssociatedTokenAddressSync(order[1], owner), creator_fee_share: share,
      token_program: TOKEN_PROGRAM_ID, associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID, system_program: SystemProgram.programId, ...overrides }, { side });
    if (tier === 13 && (side === 0 || side === 1) && Object.keys(overrides).length === 0) {
      const built = buildRaydiumCreatorFeeClaim({ program, virtualPool: pool, config: config.publicKey, raydiumPool: g.addresses.pool,
        owner, side, feeNft: nft.nft, feeNftAccount: getAssociatedTokenAddressSync(nft.nft,owner), token0Mint: order[0], token1Mint: order[1] });
      const normalize = (keys: TransactionInstruction['keys']) => keys.map(k => ({ ...k, pubkey: k.pubkey.toBase58() }));
      assert.deepEqual(normalize(built.keys),normalize(ix.keys),'backend claim builder matches compiler-generated ABI');
      assert.deepEqual(built.data,ix.data);
      return built;
    }
    return ix;
  }
  return { svm, send, payer, creator, config, mint, rayConfig, permission, graduate, g, order, trade, claim, feeAuthority,
    collected, ledger, share, readPool, pool, quoteVault, baseVault, reimbursement, params, production, common, baseAta, quoteAta };
}

test('upgrade preserves old one-percent curve with two-percent Raydium destination and both curve claims', () => {
  // The old pinned program creates the immutable 1%/#2 configuration. Context
  // then upgrades only the VM executable before initialization and the buy.
  const c = context(2, false, true, true), before = c.readPool();
  const creatorExpected = before.creator_quote_fee, partnerExpected = before.partner_quote_fee;
  assert.equal(c.params.pool_fees.base_fee.cliff_fee_numerator, 10_000_000n);
  assert.ok(creatorExpected > 0n && partnerExpected > 0n);
  const accounts = {...c.common, pool:c.pool,config:c.config.publicKey,base_mint:c.mint.publicKey,
    quote_mint:NATIVE_MINT,base_vault:c.baseVault,quote_vault:c.quoteVault};
  const creatorBefore=amount(c.svm,c.quoteAta);
  c.send([curveIx('claim_creator_trading_fee',{...accounts,creator:c.creator.publicKey,
    token_a_account:c.baseAta,token_b_account:c.quoteAta},{max_base_amount:0xffffffffffffffffn,max_quote_amount:0xffffffffffffffffn})],[c.creator]);
  assert.equal(amount(c.svm,c.quoteAta)-creatorBefore,creatorExpected);
  assert.equal(c.readPool().creator_quote_fee,0n);
  assert.equal(c.readPool().partner_quote_fee,partnerExpected);
  const treasuryBase=getAssociatedTokenAddressSync(c.mint.publicKey,treasury),treasuryQuote=getAssociatedTokenAddressSync(NATIVE_MINT,treasury);
  c.send([createAssociatedTokenAccountIdempotentInstruction(c.payer.publicKey,treasuryBase,treasury,c.mint.publicKey),
    createAssociatedTokenAccountIdempotentInstruction(c.payer.publicKey,treasuryQuote,treasury,NATIVE_MINT)]);
  const partnerBefore=amount(c.svm,treasuryQuote);
  c.send([curveIx('claim_trading_fee',{...accounts,fee_claimer:treasury,token_a_account:treasuryBase,token_b_account:treasuryQuote},
    {max_amount_a:0xffffffffffffffffn,max_amount_b:0xffffffffffffffffn})],[c.payer],false,treasury);
  assert.equal(amount(c.svm,treasuryQuote)-partnerBefore,partnerExpected);
  assert.equal(c.readPool().partner_quote_fee,0n);
  c.graduate();c.trade();c.trade(false);
  assert.equal(c.readPool().migration_progress,3);
});

for (const tier of [13, 2] as const) for (const production of [false, true]) for (const baseFirst of [false, true])
test(`actual SBF graduation #${tier}, ${production ? 85 : 0.5} SOL, baseFirst=${baseFirst}`, () => {
  const c = context(tier, production, baseFirst), before = c.readPool();
  const supplyBefore = Buffer.from(c.svm.getAccount(c.mint.publicKey)!.data).readBigUInt64LE(36);
  c.graduate(); const after = c.readPool(), r = after.graduation;
  const revenue = production ? 800_000_000n : 0n;
  assert.equal(r.quote_deposited + r.setup_lamports + revenue, c.params.migration_quote_threshold);
  assert.equal(after.partner_quote_fee - before.partner_quote_fee, revenue);
  assert.equal(after.leftover_amount, 0n);
  assert.ok(Buffer.from(c.svm.getAccount(c.mint.publicKey)!.data).readBigUInt64LE(36) < supplyBefore);
  assert.ok(r.partner_lp_amount === r.creator_lp_amount || r.partner_lp_amount === r.creator_lp_amount + 1n);
  const cp = Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data);
  assert.equal(cp[390], tier === 13 ? 1 : 0);
  assert.ok(new PublicKey(cp.subarray(40,72)).equals(tier === 13 ? c.feeAuthority : authority));
  const saved = Buffer.from(c.svm.getAccount(c.pool)!.data); c.graduate(true);
  assert.deepEqual(Buffer.from(c.svm.getAccount(c.pool)!.data), saved);
  c.trade(); c.trade(false);
  if (tier === 2) {
    assert.equal(Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data).readBigUInt64LE(397), 0n);
    c.send([c.claim(0)], [c.creator], true);
  }
});

for (const baseFirst of [false,true]) test(`creator claims split net receipts and follow NFT transfers, baseFirst=${baseFirst}`, () => {
  const c = context(13,false,baseFirst); c.graduate(); c.trade(); c.trade(false);
  const cpBefore = Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data);
  const gross = [cpBefore.readBigUInt64LE(397), cpBefore.readBigUInt64LE(405)]; assert.ok(gross.every(n => n > 0n));
  const net = gross.map(n => n - n * 50_000n / 1_000_000n);
  const attacker = Keypair.generate(); c.svm.airdrop(attacker.publicKey, 5_000_000_000n);
  c.send([c.claim(0, attacker.publicKey, { fee_nft_account: getAssociatedTokenAddressSync(c.g.addresses.positions[0].nft, c.creator.publicKey) })], [attacker], true);
  c.send([c.claim(0, c.creator.publicKey, { destination_0: getAssociatedTokenAddressSync(c.order[0], attacker.publicKey) })], [c.creator], true);
  c.send([c.claim(2,c.creator.publicKey)], [c.creator], true);
  const dest = [c.creator.publicKey, treasury].map(o => c.order.map(m => getAssociatedTokenAddressSync(m, o)));
  const before = dest.map(row => row.map(a => amount(c.svm,a)));
  c.send([c.claim(0)], [c.creator]);
  c.send([c.claim(1)], [c.payer], false, treasury);
  const payout = dest.map((row,s) => row.map((a,t) => amount(c.svm,a) - before[s][t]));
  assert.deepEqual(payout[0], net.map(n => n / 2n)); assert.deepEqual(payout[1], net.map(n => n - n / 2n));
  assert.deepEqual(c.collected.map(a => amount(c.svm,a)), [0n,0n]);
  const ledger = Buffer.from(c.svm.getAccount(c.ledger)!.data);
  c.send([c.claim(0)], [c.creator]); assert.deepEqual(Buffer.from(c.svm.getAccount(c.ledger)!.data), ledger);
  c.trade();
  const nft = c.g.addresses.positions[0].nft, old = getAssociatedTokenAddressSync(nft,c.creator.publicKey), next = getAssociatedTokenAddressSync(nft,attacker.publicKey);
  c.send([createAssociatedTokenAccountIdempotentInstruction(c.creator.publicKey,next,attacker.publicKey,nft),
    createTransferInstruction(old,next,c.creator.publicKey,1n)], [c.creator]);
  c.send([c.claim(0)], [c.creator], true);
  c.send([c.claim(0,attacker.publicKey)], [attacker]);
  assert.ok(c.order.map(m => amount(c.svm,getAssociatedTokenAddressSync(m,attacker.publicKey))).some(n => n > 0n));
});

for (const tier of [13,2] as const) test(`LP claims remain independent and preserve locked principal, config #${tier}`, () => {
  const c = context(tier); c.graduate(); c.trade(); c.trade(false);
  const lockAuthority = pda(locker,'lock_cp_authority_seed');
  const destinations = [c.creator.publicKey,treasury].map(owner => c.order.map(m => getAssociatedTokenAddressSync(m,owner)));
  for(const [i,owner] of [c.creator.publicKey,treasury].entries()) c.send(c.order.map((m,j) =>
    createAssociatedTokenAccountIdempotentInstruction(c.payer.publicKey,destinations[i][j],owner,m)));
  const claimLp = (side:number) => {
    const owner = side === 0 ? c.creator.publicKey : treasury, position = c.g.addresses.positions[side];
    const keys = [lockAuthority,owner,getAssociatedTokenAddressSync(position.nft,owner),position.lock,cpmm,cpAuthority,
      c.g.addresses.pool,c.g.addresses.lpMint,...destinations[side],...c.order.map(m=>c.g.addresses.vault(m)),...c.order,
      c.g.addresses.lockedLpVault,TOKEN_PROGRAM_ID,TOKEN_2022_PROGRAM_ID,new PublicKey('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr')];
    const data = Buffer.alloc(16); Buffer.from([8,30,51,199,209,184,247,133]).copy(data); data.writeBigUInt64LE(0xffffffffffffffffn,8);
    return new TransactionInstruction({programId:locker,data,keys:keys.map((pubkey,i)=>({pubkey,isSigner:i===1,isWritable:[2,3,6,7,8,9,10,11,14].includes(i)}))});
  };
  const cp = () => Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data);
  const creatorLiabilities = [cp().readBigUInt64LE(397),cp().readBigUInt64LE(405)];
  const before = destinations.map(row=>row.map(a=>amount(c.svm,a)));
  c.send([claimLp(0)],[c.creator]); c.send([claimLp(1)],[c.payer],false,treasury);
  destinations.forEach((row,s)=>row.forEach((a,t)=>assert.ok(amount(c.svm,a)>before[s][t])));
  assert.deepEqual([cp().readBigUInt64LE(397),cp().readBigUInt64LE(405)],creatorLiabilities,'LP claim does not consume creator bucket');
  const vault = c.g.addresses.lockedLpVault, lpBefore=amount(c.svm,vault);
  const userLp=getAssociatedTokenAddressSync(c.g.addresses.lpMint,c.creator.publicKey);
  c.send([createAssociatedTokenAccountIdempotentInstruction(c.creator.publicKey,userLp,c.creator.publicKey,c.g.addresses.lpMint)], [c.creator]);
  c.send([createTransferInstruction(vault,userLp,c.creator.publicKey,1n)], [c.creator],true);
  assert.equal(amount(c.svm,vault),lpBefore);
  if(tier===13){c.send([c.claim(0)],[c.creator]);c.send([c.claim(1)],[c.payer],false,treasury);}
});

test('permissionless external collection before our claim is accounted for; pools stay isolated', () => {
  const c=context();c.graduate();c.trade();c.trade(false);
  const cp=Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data);
  const net=[cp.readBigUInt64LE(397),cp.readBigUInt64LE(405)].map(n=>n-n*50_000n/1_000_000n);
  const keys=[c.payer.publicKey,c.feeAuthority,cpAuthority,c.g.addresses.pool,
    ...c.order.map(m=>c.g.addresses.vault(m)),...c.order,...c.collected,TOKEN_PROGRAM_ID,TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,SystemProgram.programId,c.rayConfig,c.share];
  c.send([new TransactionInstruction({programId:cpmm,data:disc('global:collect_creator_fee_permissionless'),
    keys:keys.map((pubkey,i)=>({pubkey,isSigner:i===0,isWritable:[0,3,4,5,8,9].includes(i)}))})]);
  assert.deepEqual(c.collected.map(a=>amount(c.svm,a)),net);
  const otherAuthority=pda(program,'ray_creator_fees',Keypair.generate().publicKey.toBuffer());
  c.send([c.claim(0,c.creator.publicKey,{fee_authority:otherAuthority})],[c.creator],true);
  c.send([c.claim(0)],[c.creator]);
  assert.deepEqual(c.collected.map(a=>amount(c.svm,a)),net.map(n=>n-n/2n));
  c.send([c.claim(1)],[c.payer],false,treasury);
  assert.deepEqual(c.collected.map(a=>amount(c.svm,a)),[0n,0n]);
});

test('one holder can claim both creator Fee Keys within the app compute and packet budgets',()=>{
  const c=context();c.graduate();c.trade();c.trade(false);
  const nft=c.g.addresses.positions[1].nft,from=getAssociatedTokenAddressSync(nft,treasury),to=getAssociatedTokenAddressSync(nft,c.creator.publicKey);
  c.send([createAssociatedTokenAccountIdempotentInstruction(c.payer.publicKey,to,c.creator.publicKey,nft),
    createTransferInstruction(from,to,treasury,1n)],[c.payer],false,treasury);
  const tx=new Transaction({feePayer:c.creator.publicKey,recentBlockhash:c.svm.latestBlockhash()})
    .add(ComputeBudgetProgram.setComputeUnitLimit({units:1_000_000}),ComputeBudgetProgram.setComputeUnitPrice({microLamports:1000}),
      c.claim(0),c.claim(1,c.creator.publicKey));
  tx.sign(c.creator);assert.ok(tx.serialize().length<=1232);
  const result=c.svm.sendTransaction(tx);
  if(result instanceof FailedTransactionMetadata)assert.fail(String(result.err())+'\n'+result.meta().logs().join('\n'));
  assert.ok(result.computeUnitsConsumed()<1_000_000n);
  assert.deepEqual(c.collected.map(a=>amount(c.svm,a)),[0n,0n]);
});

for (const rate of [0n,333_333n,1_000_000n,1_000_001n]) test(`creator-share override ${rate}: split only net, invalid rate rolls back`, () => {
  const c=context(); c.graduate(); c.trade(); c.trade(false);
  const before=Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data);
  const gross=[before.readBigUInt64LE(397),before.readBigUInt64LE(405)];
  const data=Buffer.alloc(145); disc('account:CreatorFeeShare').copy(data);
  data[8]=PublicKey.findProgramAddressSync([Buffer.from('creator_fee_share'),c.feeAuthority.toBuffer(),c.rayConfig.toBuffer()],cpmm)[1];
  c.feeAuthority.toBuffer().copy(data,9); c.rayConfig.toBuffer().copy(data,41); data.writeBigUInt64LE(rate,73);
  c.svm.setAccount(c.share,{owner:cpmm,data,lamports:3_000_000,executable:false,rentEpoch:0});
  if(rate>1_000_000n){
    c.send([c.claim(0)],[c.creator],true);
    assert.deepEqual(Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data),before);
    assert.equal(c.svm.getAccount(c.ledger),null,'failed claim must not initialize ledger');
    return;
  }
  const destinations=[c.creator.publicKey,treasury].map(o=>c.order.map(m=>getAssociatedTokenAddressSync(m,o)));
  const balances=destinations.map(row=>row.map(a=>amount(c.svm,a)));
  // Platform first to exercise the opposite claim order with external overrides.
  c.send([c.claim(1)],[c.payer],false,treasury); c.send([c.claim(0)],[c.creator]);
  const net=gross.map(n=>n-n*rate/1_000_000n);
  destinations.forEach((row,s)=>row.forEach((a,t)=>assert.equal(amount(c.svm,a)-balances[s][t],s===0?net[t]/2n:net[t]-net[t]/2n)));
  assert.deepEqual(c.collected.map(a=>amount(c.svm,a)),[0n,0n]);
});

for (const invalid of ['permission','fee-rate'] as const) test(`graduation fails closed on changed ${invalid}`, () => {
  const c=context(), key=invalid==='permission'?c.permission:c.rayConfig;
  const account=c.svm.getAccount(key)!,data=Buffer.from(account.data);
  if(invalid==='permission') Keypair.generate().publicKey.toBuffer().copy(data,8);
  else data.writeBigUInt64LE(9701n,108);
  c.svm.setAccount(key,{...account,data});
  const before=Buffer.from(c.svm.getAccount(c.pool)!.data);
  c.graduate(true);
  assert.equal(c.svm.getAccount(c.g.addresses.pool),null);
  assert.deepEqual(Buffer.from(c.svm.getAccount(c.pool)!.data),before);
});

test('cold admin, treasury and operator permissions are separated on mainnet', () => {
  const c=context(),operator=Keypair.generate().publicKey;
  const operational=pda(program,'operational_authority');
  const initialize=(signer:PublicKey)=>curveIx('initialize_operational_authority',{
    operational_authority:operational,admin:signer,system_program:SystemProgram.programId},{operator});
  c.send([initialize(treasury)],[c.payer],true,treasury);
  c.send([initialize(c.payer.publicKey)],[c.payer],true);
  c.send([initialize(admin)],[c.payer],false,admin);
  const rotate=(signer:PublicKey,next:PublicKey)=>curveIx('set_operational_authority',{
    operational_authority:operational,admin:signer},{operator:next});
  c.send([rotate(admin,treasury)],[c.payer],true,admin);
  c.send([rotate(admin,admin)],[c.payer],true,admin);
  c.send([rotate(admin,PublicKey.default)],[c.payer],false,admin);
});

test('late graduation failure rolls back CPMM initialization, locks, revenue and burn', () => {
  const c = context(13,true);
  const before = [c.pool,c.baseVault,c.quoteVault,c.mint.publicKey].map(a => Buffer.from(c.svm.getAccount(a)!.data));
  // A synthetic frozen destination passes account identity checks but rejects
  // reimbursement after CPMM initialization and both locking CPIs execute.
  const reimbursement=c.svm.getAccount(c.reimbursement)!,data=Buffer.from(reimbursement.data);
  data[108]=2; // SPL AccountState::Frozen, VM-only fault injection.
  c.svm.setAccount(c.reimbursement,{...reimbursement,data});
  const result=c.graduate(true) as FailedTransactionMetadata;
  const logs=result.meta().logs().join('\n');
  assert.match(logs,/Instruction: InitializeWithPermission/);
  assert.match(logs,/Instruction: LockCpLiquidity/,'failure must occur after at least one lock CPI');
  assert.equal(c.svm.getAccount(c.g.addresses.pool),null);
  [c.pool,c.baseVault,c.quoteVault,c.mint.publicKey].forEach((a,i) => assert.deepEqual(Buffer.from(c.svm.getAccount(a)!.data),before[i]));
});
