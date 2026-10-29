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
// Isolated internal review additions, 2026-10-29. No network/chain writes.
assert.equal(createHash('sha256').update(readFileSync(contractMainnetBinaryUrl)).digest('hex'),
  '885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6');

for (const index of [8,9,10,12,13,15,24,25,28,29,31,39,40]) test(`audit: graduation rejects substituted account ${index} atomically`, () => {
  const c=context();
  const before=[c.pool,c.baseVault,c.quoteVault,c.mint.publicKey].map(a=>Buffer.from(c.svm.getAccount(a)!.data));
  const wrong=Keypair.generate().publicKey;
  c.svm.airdrop(wrong,1_000_000n);
  c.g.instruction.keys[index]={...c.g.instruction.keys[index],pubkey:index===9||index===10?SystemProgram.programId:wrong};
  c.graduate(true);
  assert.equal(c.svm.getAccount(c.g.addresses.pool),null);
  [c.pool,c.baseVault,c.quoteVault,c.mint.publicKey].forEach((a,i)=>assert.deepEqual(Buffer.from(c.svm.getAccount(a)!.data),before[i]));
});

for (const field of ['raydium_pool','fee_lock','fee_nft_account','token_0_vault','fee_authority','ledger','collected_0','destination_0'])
test(`audit: creator claim rejects substituted ${field} without collecting or initializing ledger`,()=>{
  const c=context();c.graduate();c.trade();
  const cpBefore=Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data);
  const wrong=Keypair.generate().publicKey;c.svm.airdrop(wrong,1_000_000n);
  c.send([c.claim(0,c.creator.publicKey,{[field]:wrong})],[c.creator],true);
  assert.deepEqual(Buffer.from(c.svm.getAccount(c.g.addresses.pool)!.data),cpBefore);
  assert.equal(c.svm.getAccount(c.ledger),null);
});

test('audit: base-token donation survives graduation burn; issued unused inventory burns',()=>{
  const c=context(13,true),donation=12345n;
  c.send([createTransferInstruction(c.baseAta,c.baseVault,c.creator.publicKey,donation)],[c.creator]);
  const before=c.readPool();
  const beforeSupply=Buffer.from(c.svm.getAccount(c.mint.publicKey)!.data).readBigUInt64LE(36);
  c.graduate();const after=c.readPool();
  const burn=before.base_reserve-after.graduation.base_deposited;
  assert.equal(Buffer.from(c.svm.getAccount(c.mint.publicKey)!.data).readBigUInt64LE(36),beforeSupply-burn);
  assert.equal(amount(c.svm,c.baseVault),donation);
  assert.equal(after.leftover_amount,0n);
});

test('audit: prefunding fresh Raydium pool reduces reimbursement by exact donation',()=>{
  const c=context();
  const rent=BigInt(c.svm.minimumBalanceForRentExemption(637n));
  c.send([SystemProgram.transfer({fromPubkey:c.payer.publicKey,toPubkey:c.g.addresses.pool,lamports:1n})]);
  c.graduate();
  const measured=c.readPool().graduation.setup_lamports;
  // CP create fee plus the 13 account rent shortfalls, with one donated lamport.
  const sizes=[637,82,165,165,165,4075,82,165,256,165,82,165,256];
  const fee=Buffer.from(c.svm.getAccount(c.rayConfig)!.data).readBigUInt64LE(36);
  const forecast=sizes.reduce((sum,size)=>sum+BigInt(c.svm.minimumBalanceForRentExemption(BigInt(size))),fee)-1n;
  assert.ok(rent>0n);assert.equal(measured,forecast);
  assert.equal(amount(c.svm,c.reimbursement),measured);
});
