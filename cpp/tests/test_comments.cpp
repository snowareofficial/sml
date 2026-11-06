// SPDX-License-Identifier: MulanPSL-2.0
// test_comments.cpp - mirrors rust/tests/comments.rs behaviour.
#include "sml.hpp"
#include <iostream>
#include <cassert>
#include "string"

using namespace sml;
static int failures=0;
#define CHECK(c,m) do{ if(!(c)){ std::cerr<<"FAIL: "<<m<<"\n"; failures++; } }while(0)
static ValuePtr ok(const std::string& t){ std::string e; auto v=Parser::parse(t,&e); if(!v){std::cerr<<"PARSE FAIL: "<<e<<"\n";failures++;} return v; }
static std::string gs(const ValuePtr& v,const std::string&k){ auto x=v->get(k); return x&&x->tag==Value::Tag::Str?x->s:""; }
static long long gi(const ValuePtr& v,const std::string&k){ auto x=v->get(k); return x&&x->tag==Value::Tag::Int?x->i:-99999; }

int main(){
    { auto v=ok("a: 1 # 行尾注释\nb: 2 # 另一个"); CHECK(gi(v,"a")==1,"hash a"); CHECK(gi(v,"b")==2,"hash b"); }
    { auto v=ok("a: 1 -- 行尾注释\nb: 2 -- 另一个"); CHECK(gi(v,"a")==1,"dash a"); CHECK(gi(v,"b")==2,"dash b"); }
    { auto v=ok("a: 1 // C style\nb: 2 // 另一个"); CHECK(gi(v,"a")==1,"slash a"); CHECK(gi(v,"b")==2,"slash b"); }
    { auto v=ok("/*\n  multi\n  a: 999\n*/\na: 1\nb: 2"); CHECK(gi(v,"a")==1,"block a"); CHECK(gi(v,"b")==2,"block b"); }
    { auto v=ok("_* another\n   multi\n*_\na: 1\nb: 2"); CHECK(gi(v,"a")==1,"ustar a"); CHECK(gi(v,"b")==2,"ustar b"); }
    { auto v=ok("server {\n port: 8080 -- 端口\n /* hosts */\n hosts: [\n  a -- 主\n  b # 备\n ]\n}");
      CHECK(gi(v->get("server"),"port")==8080,"inblock port");
      auto h=v->get("server")->get("hosts"); CHECK(h&&h->tag==Value::Tag::Arr&&h->arr.size()==2,"hosts arr");
      CHECK(h->arr[0]->s=="a"&&h->arr[1]->s=="b","hosts vals"); }
    { auto v=ok("a: -5\nb: my-word"); CHECK(gi(v,"a")==-5,"neg"); CHECK(gs(v,"b")=="my-word","word"); }
    { auto v=ok("path: a/b/c"); CHECK(gs(v,"path")=="a/b/c","path slash"); }
    { auto v=ok("id: foo_bar"); CHECK(gs(v,"id")=="foo_bar","underscore"); }
    if(failures==0) std::cout<<"ALL COMMENT TESTS PASSED\n"; else { std::cout<<failures<<" FAILURES\n"; return 1; }
    return 0;
}
