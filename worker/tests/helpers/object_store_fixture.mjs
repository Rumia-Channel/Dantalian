// Local HTTP-contract transport only; this does not verify Wasabi credentials or SigV4.
import http from 'node:http';
import { createHash } from 'node:crypto';
const Objects = new Map();
const Server = http.createServer(async (Request, Response) => {
    const Key = new URL(Request.url, 'http://localhost').pathname;
    const Cors = { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'GET,HEAD,PUT,DELETE,OPTIONS', 'Access-Control-Allow-Headers': 'Range,Content-Type,Authorization,x-amz-content-sha256,x-amz-date', 'Access-Control-Expose-Headers': 'Content-Range,Accept-Ranges,Content-Length,ETag' };
    if(Request.method === 'OPTIONS') { Response.writeHead(204, Cors); Response.end(); return; }
    if(Request.method === 'PUT') {
        const Chunks = []; for await(const Chunk of Request) Chunks.push(Chunk);
        const Bytes = Buffer.concat(Chunks), ETag = '"' + createHash('md5').update(Bytes).digest('hex') + '"';
        Objects.set(Key, { Bytes, ETag, Type: Request.headers['content-type'] || 'application/octet-stream' }); Response.writeHead(200, { ...Cors, ETag }); Response.end(); return;
    }
    if(Request.method === 'DELETE') { Objects.delete(Key); Response.writeHead(204, Cors); Response.end(); return; }
    const Object = Objects.get(Key);
    if(!Object) { Response.writeHead(404, Cors); Response.end(); return; }
    const Headers = { ...Cors, 'Content-Type': Object.Type, 'Accept-Ranges': 'bytes', ETag: Object.ETag };
    let Start = 0, End = Object.Bytes.length - 1, Status = 200;
    if(Request.headers.range) {
        const Range = /^bytes=(\d*)-(\d*)$/.exec(Request.headers.range);
        if(!Range) { Response.writeHead(416, Headers); Response.end(); return; }
        Start = Range[1] ? Number(Range[1]) : Math.max(0, Object.Bytes.length - Number(Range[2]));
        End = Range[1] && Range[2] ? Math.min(Number(Range[2]), End) : End;
        if(Start > End || Start > Object.Bytes.length - 1) { Response.writeHead(416, Headers); Response.end(); return; }
        Status = 206; Headers['Content-Range'] = `bytes ${Start}-${End}/${Object.Bytes.length}`;
    }
    Headers['Content-Length'] = End - Start + 1; Response.writeHead(Status, Headers);
    Response.end(Request.method === 'HEAD' ? undefined : Object.Bytes.subarray(Start, End + 1));
});
const Port = Number(process.env.OBJECT_FIXTURE_PORT || 8795);
Server.listen(Port, '127.0.0.1', () => console.log('Object transport fixture ready :' + Port));
