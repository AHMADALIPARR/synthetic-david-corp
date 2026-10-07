import assert from 'node:assert/strict';
const endpoint='http://127.0.0.1:1235/v1/responses';
async function request(body){return fetch(endpoint,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(110000)});}
const text=await request({model:'qwen-local',input:'Reply with exactly OK. /no_think',max_output_tokens:32,stream:true});
assert.equal(text.status,200);assert.match(text.headers.get('content-type'),/text\/event-stream/);
const events=(await text.text()).split('\n').filter(line=>line.startsWith('data: ')).map(line=>JSON.parse(line.slice(6)));
assert.ok(events.some(e=>e.type==='response.output_text.delta'));
const completed=events.find(e=>e.type==='response.completed');assert.ok(completed.response.output.some(item=>item.type==='message'));assert.ok(completed.response.usage.total_tokens>0);
console.log('Real Qwen -> Rust -> Codex Responses SSE text and usage: passed');
const tool=await request({model:'qwen-local',input:'Call evidence_check with input equal to test-fixture. /no_think',max_output_tokens:160,tools:[{type:'custom',name:'evidence_check',description:'Development wire fixture; accepts a short text string.'}],tool_choice:{type:'custom',name:'evidence_check'}});
assert.equal(tool.status,200);
const toolResponse=await tool.json();const call=toolResponse.output.find(item=>item.type==='custom_tool_call');assert.equal(call?.name,'evidence_check');assert.ok(typeof call.input==='string');
// Report a labeled wire fixture result; no tool or bank action is executed.
const continued=await request({model:'qwen-local',input:[{role:'user',content:'Check the development wire fixture.'},call,{type:'custom_tool_call_output',call_id:call.call_id,output:'DEVELOPMENT-WIRE-FIXTURE-OK'},{role:'user',content:'Reply with exactly OK. /no_think'}],max_output_tokens:32});
assert.equal(continued.status,200);assert.ok((await continued.json()).output.some(item=>item.type==='message'));
console.log('Real Qwen custom tool translation and result continuation: passed (no tool execution)');
const denied=await request({model:'unapproved-model',input:'hello'});assert.equal(denied.status,400);
console.log('Unknown model rejected: passed');
