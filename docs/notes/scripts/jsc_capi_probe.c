// Probe for docs/notes/apple-gui-backend.md §8 (JSC C API, WebKitGTK or Apple framework).
// Minimal JSC C-API runner (WebKitGTK javascriptcoregtk-4.1): print(), file eval, microtask probe.
#include <JavaScriptCore/JavaScript.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char *slurp(const char *p){FILE*f=fopen(p,"rb");if(!f){perror(p);exit(1);}fseek(f,0,2);long n=ftell(f);rewind(f);char*b=malloc(n+1);fread(b,1,n,f);b[n]=0;fclose(f);return b;}
static JSValueRef js_print(JSContextRef c, JSObjectRef fn, JSObjectRef th, size_t argc, const JSValueRef a[], JSValueRef *exc){
  for(size_t i=0;i<argc;i++){JSStringRef s=JSValueToStringCopy(c,a[i],exc);char buf[4096];JSStringGetUTF8CString(s,buf,sizeof buf);printf("%s%s",i?" ":"",buf);JSStringRelease(s);} printf("\n"); return JSValueMakeUndefined(c);}
static JSValueRef js_reenter(JSContextRef c, JSObjectRef fn, JSObjectRef th, size_t argc, const JSValueRef a[], JSValueRef *exc){
  // native op that synchronously calls back into JS (like a toolkit emitting a sync signal during insert)
  JSObjectCallAsFunction(c,(JSObjectRef)a[0],NULL,0,NULL,exc);
  JSStringRef s=JSStringCreateWithUTF8CString("print('  native: after nested call, log =', JSON.stringify(log))");
  JSEvaluateScript(c,s,NULL,NULL,1,exc); JSStringRelease(s); return JSValueMakeUndefined(c);}
static void ev(JSGlobalContextRef ctx,const char*src,const char*name){JSStringRef s=JSStringCreateWithUTF8CString(src),u=JSStringCreateWithUTF8CString(name);JSValueRef exc=NULL;JSEvaluateScript(ctx,s,NULL,u,1,&exc);
  if(exc){JSStringRef e=JSValueToStringCopy(ctx,exc,NULL);char b[1024];JSStringGetUTF8CString(e,b,sizeof b);fprintf(stderr,"exception: %s\n",b);}JSStringRelease(s);JSStringRelease(u);}
int main(int argc,char**argv){
  JSGlobalContextRef ctx=JSGlobalContextCreate(NULL);
  JSStringRef pn=JSStringCreateWithUTF8CString("print");
  JSObjectSetProperty(ctx,JSContextGetGlobalObject(ctx),pn,JSObjectMakeFunctionWithCallback(ctx,pn,js_print),0,NULL);
  if(argc>1 && !strcmp(argv[1],"--microtask")){
    ev(ctx,"globalThis.log=[]; Promise.resolve().then(()=>log.push('job1')); log.push('sync');","a.js");
    ev(ctx,"print('after eval #1 returned:', JSON.stringify(log))","b.js");
    ev(ctx,"globalThis.f=function(){ Promise.resolve().then(()=>log.push('job-from-call')); log.push('call'); }","c.js");
    JSStringRef fn=JSStringCreateWithUTF8CString("f");
    JSObjectRef f=(JSObjectRef)JSObjectGetProperty(ctx,JSContextGetGlobalObject(ctx),fn,NULL);
    JSObjectCallAsFunction(ctx,f,NULL,0,NULL,NULL);
    ev(ctx,"print('after JSObjectCallAsFunction returned:', JSON.stringify(log))","d.js");
    // nested: a native callback calling back into JS while JS is on the stack
    ev(ctx,"log.length=0; Promise.resolve().then(()=>log.push('outer-job')); print('inside eval, before return:', JSON.stringify(log));","e.js");
    ev(ctx,"print('next eval sees:', JSON.stringify(log))","f.js");
    JSStringRef rn=JSStringCreateWithUTF8CString("reenter");
    JSObjectSetProperty(ctx,JSContextGetGlobalObject(ctx),rn,JSObjectMakeFunctionWithCallback(ctx,rn,js_reenter),0,NULL);
    ev(ctx,"log.length=0; reenter(()=>{ Promise.resolve().then(()=>log.push('job-queued-in-nested')); log.push('nested-handler'); }); print('outer JS after native returned:', JSON.stringify(log));","g.js");
    ev(ctx,"print('after outer eval:', JSON.stringify(log))","h.js");
    return 0;}
  for(int i=1;i<argc;i++){char*s=slurp(argv[i]);ev(ctx,s,argv[i]);free(s);}
  JSGlobalContextRelease(ctx);return 0;}
