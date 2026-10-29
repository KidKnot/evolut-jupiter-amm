// Audit-only negative tests. Real pinned SBF, offline accounts, never a live RPC.
// Known cold-admin/treasury identities are substituted ONLY in setup; attackers
// and operational workers use generated keys with normal signature verification.
import test from 'node:test';
import assert from 'node:assert/strict';
import { PublicKey, Keypair, SystemProgram } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, NATIVE_MINT, getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotentInstruction } from '@solana/spl-token';
import { curveIx, decodeAccount } from './curve-idl-codec.ts';
import { context, program, treasury, admin, pda, amount, metadata } from './fixture-helper.ts';

function newLaunch(c: ReturnType<typeof context>, evolving: boolean) {
  const config = Keypair.generate(), mint = Keypair.generate();
  const configIx = curveIx('create_config', { ...c.common, config: config.publicKey,
    fee_claimer: treasury, leftover_receiver: treasury, quote_mint: NATIVE_MINT,
    raydium_amm_config: c.rayConfig, payer: c.payer.publicKey },
    { config_parameters: { ...c.params, token_update_authority: evolving ? 2 : 0 } });
  c.send([configIx], [c.payer, config], false, treasury);
  const order = [mint.publicKey, NATIVE_MINT].sort((a,b) => Buffer.compare(a.toBuffer(),b.toBuffer()));
  const pool = pda(program, 'pool', config.publicKey.toBuffer(), order[1].toBuffer(), order[0].toBuffer());
  const mintMetadata = pda(metadata, 'metadata', metadata.toBuffer(), mint.publicKey.toBuffer());
  const accounts = { ...c.common, config: config.publicKey, creator: c.creator.publicKey,
    base_mint: mint.publicKey, quote_mint: NATIVE_MINT, pool,
    base_vault: pda(program,'token_vault',mint.publicKey.toBuffer(),pool.toBuffer()),
    quote_vault: pda(program,'token_vault',NATIVE_MINT.toBuffer(),pool.toBuffer()),
    mint_metadata: mintMetadata, payer: c.payer.publicKey };
  const launch = (extra: any[] = []) => curveIx('initialize_virtual_pool_with_spl_token', accounts,
    { params: { name:'Audit evolving',symbol:'FIXED',uri:'https://example.com/original.json' } }, extra);
  return { config, mint, pool, mintMetadata, accounts, launch, configIx };
}
function setupOperator(c: ReturnType<typeof context>) {
  const operator = Keypair.generate(), operational = pda(program,'operational_authority');
  c.svm.airdrop(operator.publicKey, 100_000_000n);
  c.send([curveIx('initialize_operational_authority', { operational_authority:operational,
    admin, system_program:SystemProgram.programId }, { operator:operator.publicKey })], [c.payer], false, admin);
  return { operator, operational,
    extra:[{pubkey:operational,isSigner:false,isWritable:false},
      {pubkey:operator.publicKey,isSigner:true,isWritable:false}] };
}
function creatorClaim(c: ReturnType<typeof context>, creator = c.creator.publicKey, overrides = {}) {
  return curveIx('claim_creator_trading_fee', { ...c.common, pool:c.pool,
    base_vault:c.baseVault, quote_vault:c.quoteVault, base_mint:c.mint.publicKey,
    quote_mint:NATIVE_MINT, creator, token_a_account:c.baseAta, token_b_account:c.quoteAta,
    ...overrides }, { max_base_amount:0xffffffffffffffffn,max_quote_amount:0xffffffffffffffffn });
}
function metadataFields(raw: Uint8Array) {
  const b = Buffer.from(raw); let pos = 65;
  const str = () => {const n=b.readUInt32LE(pos);pos+=4;const s=b.subarray(pos,pos+n).toString();pos+=n;return s.replace(/\0+$/, '');};
  return { authority:new PublicKey(b.subarray(1,33)).toBase58(), mint:new PublicKey(b.subarray(33,65)).toBase58(),
    name:str(),symbol:str(),uri:str() };
}

test('unauthorized configuration creation rejects and creates no policy account', () => {
  const c=context(), config=Keypair.generate();
  const ix=curveIx('create_config',{...c.common,config:config.publicKey,fee_claimer:c.creator.publicKey,
    leftover_receiver:treasury,quote_mint:NATIVE_MINT,raydium_amm_config:c.rayConfig,payer:c.payer.publicKey},
    {config_parameters:c.params});
  c.send([ix],[c.payer,config,c.creator],true);
  assert.equal(c.svm.getAccount(config.publicKey),null);
});

test('Classic initialization is permissionless and transfers no platform launch fee', () => {
  const c=context(), launch=newLaunch(c,false), before=c.svm.getAccount(treasury)!.lamports;
  c.send([launch.launch()],[c.payer,c.creator,launch.mint]);
  assert.equal(c.svm.getAccount(treasury)!.lamports,before);
  assert.equal(decodeAccount('VirtualPool',c.svm.getAccount(launch.pool)!.data).creator.toBase58(),c.creator.publicKey.toBase58());
});

test('Evolving launch without worker or treasury signature rejects atomically', () => {
  const c=context(), launch=newLaunch(c,true);
  c.send([launch.launch()],[c.payer,c.creator,launch.mint],true);
  assert.equal(c.svm.getAccount(launch.pool),null);
  assert.equal(c.svm.getAccount(launch.mint.publicKey),null);
});

test('Evolving launch rejects nonsigner operator and wrong signed operator, then accepts canonical worker', () => {
  const c=context(), auth=setupOperator(c), launch=newLaunch(c,true), attacker=Keypair.generate();
  const unsigned=auth.extra.map(a=>({...a,isSigner:false}));
  c.send([launch.launch(unsigned)],[c.payer,c.creator,launch.mint],true);
  const wrong=[auth.extra[0],{pubkey:attacker.publicKey,isSigner:true,isWritable:false}];
  c.send([launch.launch(wrong)],[c.payer,c.creator,launch.mint,attacker],true);
  c.send([launch.launch(auth.extra)],[c.payer,c.creator,launch.mint,auth.operator]);
  assert.equal(metadataFields(c.svm.getAccount(launch.mintMetadata)!.data).authority,auth.operational.toBase58());
});

test('revoked worker cannot launch with canonical PDA, while treasury legacy path remains authorized', () => {
  const c=context(), auth=setupOperator(c), launch=newLaunch(c,true);
  c.send([curveIx('set_operational_authority',{operational_authority:auth.operational,admin},
    {operator:PublicKey.default})],[c.payer],false,admin);
  c.send([launch.launch(auth.extra)],[c.payer,c.creator,launch.mint,auth.operator],true);
  const treasuryExtra=[{pubkey:treasury,isSigner:true,isWritable:false}];
  c.send([launch.launch(treasuryExtra)],[c.payer,c.creator,launch.mint],false,treasury);
  assert.equal(metadataFields(c.svm.getAccount(launch.mintMetadata)!.data).authority,treasury.toBase58());
});

test('operational metadata update rejects unrelated wallet, preserves symbol/URI and honors rotation', () => {
  const c=context(), auth=setupOperator(c), launch=newLaunch(c,true);
  c.send([launch.launch(auth.extra)],[c.payer,c.creator,launch.mint,auth.operator]);
  const before=metadataFields(c.svm.getAccount(launch.mintMetadata)!.data);
  const update=(operator:PublicKey,name:string) => curveIx('update_operational_metadata',{
    config:launch.config.publicKey,pool:launch.pool,operational_authority:auth.operational,
    operator,mint_metadata:launch.mintMetadata,metadata_program:metadata},{name});
  c.send([update(c.creator.publicKey,'Unauthorized')],[c.payer,c.creator],true);
  c.send([update(auth.operator.publicKey,'Stage two')],[c.payer,auth.operator]);
  const after=metadataFields(c.svm.getAccount(launch.mintMetadata)!.data);
  assert.deepEqual({...after,name:before.name},before);
  assert.equal(after.name,'Stage two');
  c.send([curveIx('set_operational_authority',{operational_authority:auth.operational,admin},
    {operator:PublicKey.default})],[c.payer],false,admin);
  c.send([update(auth.operator.publicKey,'After revoke')],[c.payer,auth.operator],true);
  assert.deepEqual(metadataFields(c.svm.getAccount(launch.mintMetadata)!.data),after);
});

test('creator fee theft rejects and failed payout does not clear accrued amount', () => {
  const c=context(), before=Buffer.from(c.svm.getAccount(c.pool)!.data);
  c.send([creatorClaim(c,c.payer.publicKey)],[c.payer],true);
  assert.deepEqual(Buffer.from(c.svm.getAccount(c.pool)!.data),before);
  // Wrong-mint receiver fails inside SPL transfer; the earlier counter mutation rolls back.
  c.send([creatorClaim(c,c.creator.publicKey,{token_b_account:c.baseAta})],[c.creator],true);
  assert.deepEqual(Buffer.from(c.svm.getAccount(c.pool)!.data),before);
  const expected=c.readPool().creator_quote_fee, bal=amount(c.svm,c.quoteAta);
  c.send([creatorClaim(c)],[c.creator]);
  assert.equal(amount(c.svm,c.quoteAta)-bal,expected);
  const first=amount(c.svm,c.quoteAta);
  c.send([creatorClaim(c)],[c.creator]);
  assert.equal(amount(c.svm,c.quoteAta),first);
});

test('cross-pool vault substitution cannot withdraw another pool balance', () => {
  const c=context(), other=newLaunch(c,false);
  c.send([other.launch()],[c.payer,c.creator,other.mint]);
  const before=Buffer.from(c.svm.getAccount(c.pool)!.data);
  c.send([creatorClaim(c,c.creator.publicKey,{quote_vault:other.accounts.quote_vault})],[c.creator],true);
  assert.deepEqual(Buffer.from(c.svm.getAccount(c.pool)!.data),before);
});

test('treasury partner claims cannot redirect fees even with treasury authorization', () => {
  const c=context();
  const base=getAssociatedTokenAddressSync(c.mint.publicKey,treasury), quote=getAssociatedTokenAddressSync(NATIVE_MINT,treasury);
  c.send([createAssociatedTokenAccountIdempotentInstruction(c.payer.publicKey,base,treasury,c.mint.publicKey),
    createAssociatedTokenAccountIdempotentInstruction(c.payer.publicKey,quote,treasury,NATIVE_MINT)]);
  const ix=(destination:PublicKey) => curveIx('claim_trading_fee',{...c.common,config:c.config.publicKey,pool:c.pool,
    base_vault:c.baseVault,quote_vault:c.quoteVault,base_mint:c.mint.publicKey,quote_mint:NATIVE_MINT,
    fee_claimer:treasury,token_a_account:base,token_b_account:destination},
    {max_amount_a:0xffffffffffffffffn,max_amount_b:0xffffffffffffffffn});
  const before=c.readPool().partner_quote_fee;
  c.send([ix(c.quoteAta)],[c.payer],true,treasury);
  assert.equal(c.readPool().partner_quote_fee,before);
  c.send([ix(quote)],[c.payer],false,treasury);
  assert.equal(amount(c.svm,quote),before);
});

test('only current creator may transfer residual curve fee rights', () => {
  const c=context(); c.graduate();
  const transfer=(creator:PublicKey)=>curveIx('transfer_pool_creator',{...c.common,virtual_pool:c.pool,
    config:c.config.publicKey,creator,new_creator:c.payer.publicKey});
  c.send([transfer(c.payer.publicKey)],[c.payer],true);
  c.send([transfer(c.creator.publicKey)],[c.creator]);
  c.send([creatorClaim(c)],[c.creator],true);
  c.send([creatorClaim(c,c.payer.publicKey)],[c.payer]);
  assert.equal(c.readPool().creator.toBase58(),c.payer.publicKey.toBase58());
});
