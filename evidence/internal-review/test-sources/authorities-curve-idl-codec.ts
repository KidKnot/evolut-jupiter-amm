import { readFileSync } from 'node:fs';
import { PublicKey, TransactionInstruction } from '@solana/web3.js';
import { contractIdlUrl } from '../../matched-fee-contract-20261028/contract-artifacts.mjs';
// Test-only codec driven entirely by the generated Anchor 0.31 IDL. The workspace's
// optional Anchor JS installation is 0.29 and cannot consume the fresh IDL schema.
export const curveIdl = JSON.parse(readFileSync(process.env.EVOLUT_VM_NETWORK === 'mainnet'
  ? new URL('../../matched-fee-contract-20261028/idl/evolut_curve.mainnet.json', import.meta.url) : contractIdlUrl, 'utf8'));
export const curveProgram = new PublicKey(curveIdl.address);
const type = (name: string) => curveIdl.types.find((t: any) => t.name === name).type;
export function encodeType(t: any, value?: any): Buffer {
  if (typeof t === 'object') {
    if (t.defined) return encodeType(type(t.defined.name), value);
    if (t.array) return Buffer.concat(Array.from({ length: t.array[1] }, (_,i) => encodeType(t.array[0], value?.[i])));
    if (t.option) return value == null ? Buffer.from([0]) : Buffer.concat([Buffer.from([1]), encodeType(t.option, value)]);
    if (t.vec) return Buffer.concat([encodeType('u32', value?.length || 0), ...(value || []).map((v: any) => encodeType(t.vec,v))]);
    if (t.kind === 'struct') return Buffer.concat(t.fields.map((f: any) => encodeType(f.type, value?.[f.name])));
  }
  if (t === 'pubkey') return (value || PublicKey.default).toBuffer();
  if (t === 'string') { const bytes = Buffer.from(value || ''); return Buffer.concat([encodeType('u32',bytes.length),bytes]); }
  if (t === 'bool') return Buffer.from([Number(!!value)]);
  const sizes: any = { u8:1,u16:2,u32:4,u64:8,u128:16,i64:8 };
  if (!sizes[t]) throw new Error('Unsupported IDL type '+JSON.stringify(t));
  const b=Buffer.alloc(sizes[t]); let n=BigInt(value || 0);
  if (n < 0) n += 1n << BigInt(b.length*8);
  for (let i=0;i<b.length;i++) { b[i]=Number(n & 255n); n >>= 8n; }
  return b;
}
export function decodeType(t: any, data: Buffer, at={offset:0}): any {
  if (typeof t === 'object') {
    if (t.defined) return decodeType(type(t.defined.name),data,at);
    if (t.array) return Array.from({length:t.array[1]},()=>decodeType(t.array[0],data,at));
    if (t.option) return decodeType('u8',data,at) ? decodeType(t.option,data,at) : null;
    if (t.kind === 'struct') return Object.fromEntries(t.fields.map((f:any)=>[f.name,decodeType(f.type,data,at)]));
  }
  if (t==='pubkey') { const p=new PublicKey(data.subarray(at.offset,at.offset+32)); at.offset+=32; return p; }
  const sizes:any={u8:1,u16:2,u32:4,u64:8,u128:16,i64:8,bool:1};
  const size=sizes[t]; if (!size) throw new Error('Unsupported IDL type '+JSON.stringify(t));
  let n=0n; for(let i=0;i<size;i++) n |= BigInt(data[at.offset++]) << BigInt(i*8);
  if(t==='bool') return n !== 0n;
  if(t==='i64' && n >= 1n<<63n) n-=1n<<64n;
  return size<=4 ? Number(n) : n;
}
export function encodeAccount(name:string,value:any):Buffer {
  const account=curveIdl.accounts.find((a:any)=>a.name===name);
  return Buffer.concat([Buffer.from(account.discriminator),encodeType(type(name),value)]);
}
export function decodeAccount(name:string,data:Uint8Array):any {
  return decodeType(type(name),Buffer.from(data).subarray(8));
}
export function curveIx(name:string,accounts:Record<string,PublicKey>,args:any={},extra:any[]=[]):TransactionInstruction {
  const ix=curveIdl.instructions.find((i:any)=>i.name===name);
  const keys=ix.accounts.map((a:any)=>({pubkey:accounts[a.name] || (a.optional ? curveProgram : undefined),
    isWritable:!!a.writable,isSigner:!!a.signer}));
  if(keys.some((k:any)=>!k.pubkey)) throw new Error('Missing account for '+name);
  return new TransactionInstruction({programId:curveProgram,keys:[...keys,...extra],
    data:Buffer.concat([Buffer.from(ix.discriminator),...ix.args.map((a:any)=>encodeType(a.type,args[a.name]))])});
}
