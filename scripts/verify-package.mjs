import { readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const hash=data=>createHash('sha256').update(data).digest('hex');
const source=JSON.parse(await readFile(resolve(root,'vendor-source-manifest.json'),'utf8'));
const manifest=JSON.parse(await readFile(resolve(root,'package-manifest.json'),'utf8'));
const errors=[];
for(const [prefix,files] of [['vendor/evolut-curve',source.files],['',manifest.files]]){
  for(const entry of files){
    const path=resolve(root,prefix,entry.path),rel=relative(root,path);
    if(rel.startsWith('..'+sep)||rel==='..')throw Error('Manifest path escapes repository');
    try{if(hash(await readFile(path))!==entry.sha256)errors.push(entry.path);}catch{errors.push(entry.path);}
  }
}
const ignored=new Set(['.git','target','node_modules','build-logs','.cache']);
async function walk(dir){
  const paths=[];
  for(const e of await readdir(dir,{withFileTypes:true})){
    if(ignored.has(e.name))continue;
    const full=resolve(dir,e.name);
    if(e.isSymbolicLink())throw Error('Unexpected symlink in release tree');
    if(e.isDirectory())paths.push(...await walk(full));else paths.push(relative(root,full).split(sep).join('/'));
  }
  return paths;
}
const expected=new Set([...manifest.files.map(e=>e.path),'package-manifest.json']);
for(const path of await walk(root))if(!expected.has(path))errors.push(`unlisted:${path}`);
if(errors.length){console.error(JSON.stringify({verified:false,files:errors}));process.exitCode=1;}
else console.log(JSON.stringify({verified:true,packagedFiles:manifest.files.length,sourceFiles:source.files.length,sourceStatus:source.status,unchangedFromBaseline:source.unchangedFromBaseline,networkCalls:0}));
