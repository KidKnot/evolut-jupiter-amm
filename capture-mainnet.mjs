// Read-only public-chain fixture capture. Never signs or submits transactions.
import { createHash } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import { PublicKey } from '@solana/web3.js';
// This checkout deliberately tests a local candidate against captured accounts.
// A live capture must not silently replace its reviewed executable identity.
throw Error('Capture in the separate baseline checkout; this candidate fixture is locally built and pinned.');
const program='EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ';
const expectedHash='885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6';
const clock='SysvarC1ock11111111111111111111111111111111';
const out=new URL('./tests/fixtures/captured/',import.meta.url);
const key=(d,o)=>new PublicKey(d.subarray(o,o+32)).toBase58();
const check=(ok,message)=>{if(!ok)throw Error(message);};
if(process.argv.length>2) throw Error('No CLI arguments are accepted; configure the RPC through EVOLUT_FIXTURE_RPC_URL.');
const rpcUrl=process.env.EVOLUT_FIXTURE_RPC_URL || 'https://api.mainnet-beta.solana.com';
let requests=0;
let phase='genesis';
async function rpc(method,params){
  check(['getGenesisHash','getProgramAccounts','getMultipleAccounts'].includes(method),'Read-only methods only');
  check(++requests<=8,'RPC request bound exceeded');
  const response=await fetch(rpcUrl,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:requests,method,params}),signal:AbortSignal.timeout(20000)});
  check(response.ok,'RPC HTTP request failed');
  const body=await response.text();check(body.length<=8e6,'RPC response exceeded fixture bound');
  // RPC uses u64::MAX for exempt rent epochs. Preserve its integer text in JS.
  const json=JSON.parse(body.replace(/"rentEpoch"\s*:\s*(\d+)/g,'"rentEpoch":"$1"'));check(!json.error,`RPC returned error code ${Number(json.error?.code)}`);return json.result;
}
try {
  check(await rpc('getGenesisHash',[])==='5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d','Mainnet genesis mismatch');
  phase='pool discovery';
  const list=await rpc('getProgramAccounts',[program,{encoding:'base64',commitment:'finalized',withContext:true,filters:[{dataSize:760}]}]);
  check(list.value.length<=128,'Too many pools for bounded capture');
  const discriminator=createHash('sha256').update('account:VirtualPool').digest().subarray(0,8);
  const pools=list.value.map(p=>{
    const data=Buffer.from(p.account.data[0],'base64');
    check(data.subarray(0,8).equals(discriminator)&&p.account.owner===program,'Unexpected pool layout');
    return {address:p.pubkey,config:key(data,72),mint:key(data,136),baseVault:key(data,168),quoteVault:key(data,200),quoteReserve:data.readBigUInt64LE(240).toString(),progress:data[308],account:p.account};
  });
  const active=pools.filter(p=>p.progress===0).slice(0,8);
  const selected=active.length?active:pools.slice(0,1);
  const programData='8yQweA8Y2u7EcfAorAhsZxhk9TdQszLLjmJYrM2A8AW4';
  const productionConfigs=['FxEFJfTWdptZdNFyP9epc66rhFtXjtV6nf4Yn1wMqQw7','8HnddjgrXR45BzP1fgo3oLUc5xjP1JRkxF9DR8Wmr2uR','HFCAXuDGmKuorVoeQaYsrq9WApVbojhkQbY4KRs8uANb','DtuGZfbhKivNpowa15dBeGF2jRWKUx37L9apFqLz4Z5a'];
  const addresses=[...new Set([clock,program,programData,...pools.flatMap(p=>[p.address,p.config,p.mint,p.baseVault,p.quoteVault]),...productionConfigs,'So11111111111111111111111111111111111111112'])];
  check(addresses.length<=100,'Account batch bound exceeded');
  phase='account snapshot';
  const snapshot=await rpc('getMultipleAccounts',[addresses,{encoding:'base64',commitment:'finalized',minContextSlot:list.context.slot}]);
  const accounts=Object.fromEntries(addresses.map((a,i)=>[a,snapshot.value[i]]).filter(([,a])=>a));
  phase='program identity';
  const loader='BPFLoaderUpgradeab1e11111111111111111111111';
  const programAccount=accounts[program],dataAccount=accounts[programData];
  check(programAccount?.owner===loader&&programAccount.executable&&dataAccount?.owner===loader&&!dataAccount.executable,'Unexpected program loader identity');
  const programBytes=Buffer.from(programAccount.data[0],'base64');
  check(programBytes.length===36&&programBytes.readUInt32LE(0)===2&&key(programBytes,4)===programData,'ProgramData pointer mismatch');
  const pd=Buffer.from(accounts[programData].data[0],'base64');
  check(pd.length>=45+1207880&&pd.readUInt32LE(0)===3&&pd[12]<=1,'Invalid ProgramData layout');
  phase='program verification';
  const binary=pd.subarray(45,45+1207880);
  check(createHash('sha256').update(binary).digest('hex')===expectedHash&&pd.subarray(45+1207880).every(b=>b===0),'Deployed bytecode changed; update reviewed artifact pin first');
  const payload={capturedAt:new Date().toISOString(),network:'mainnet-beta',commitment:'finalized',slot:snapshot.context.slot,program,programSha256:expectedHash,pools:pools.map(({account,...p})=>p),selected:selected.map(p=>p.address),accounts:Object.fromEntries(Object.entries(accounts).filter(([a])=>a!==programData&&a!==program)),rpcRequests:requests,transactionsSubmitted:0};
  phase='fixture serialization';
  await mkdir(out,{recursive:true});
  await writeFile(new URL('mainnet.json',out),JSON.stringify(payload,null,2)+'\n');
  await writeFile(new URL('../evolut.so',out),binary);
  for(const pool of selected){
    const dir=new URL(`../accounts/${pool.address}/`,out);await mkdir(dir,{recursive:true});
    for(const [address,account] of Object.entries(payload.accounts)){
      const data=Buffer.from(account.data[0],'base64');
      // bincode 1.x Account: lamports/u64, Vec length/u64 + data, owner[32], bool/u8, rent_epoch/u64.
      const encoded=Buffer.alloc(8+8+data.length+32+1+8);
      encoded.writeBigUInt64LE(BigInt(account.lamports),0);encoded.writeBigUInt64LE(BigInt(data.length),8);
      data.copy(encoded,16);new PublicKey(account.owner).toBuffer().copy(encoded,16+data.length);
      encoded[48+data.length]=account.executable?1:0;encoded.writeBigUInt64LE(BigInt(account.rentEpoch),49+data.length);
      await writeFile(new URL(`${address}.bin`,dir),encoded);
    }
  }
  console.log(JSON.stringify({slot:payload.slot,pools:payload.pools,selected:payload.selected,rpcRequests:requests,programHashVerified:true,transactionsSubmitted:0}));
}catch(error){console.error(JSON.stringify({failed:true,phase,requests,kind:error.name,rpcCode:/^RPC returned error code -?\d+$/.test(error.message)?error.message:undefined,credentialsLogged:false}));process.exitCode=1;}
