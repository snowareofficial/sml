// SPDX-License-Identifier: MulanPSL-2.0
// test_contracts.cpp - mirrors rust/tests/contract.rs behaviour.
#include "sml.hpp"
#include <iostream>
#include <cassert>
#include <string>

using namespace sml;

static int failures = 0;
#define CHECK(cond, msg) do { if(!(cond)){ std::cerr<<"FAIL: "<<msg<<"\n"; failures++; } } while(0)

// parse, expect success
static ValuePtr ok(const std::string& t){ std::string e; auto v=Parser::parse(t,&e); if(!v){std::cerr<<"PARSE FAIL: "<<e<<"\n";failures++;} return v; }
// parse, expect failure; return error string
static std::string err(const std::string& t){ std::string e; auto v=Parser::parse(t,&e); if(v){std::cerr<<"SHOULD FAIL but parsed\n";failures++;} return e; }
static std::string get_str(const ValuePtr& v, const std::string& k){
    // support dot path
    if (!v) return "";
    ValuePtr cur=v; size_t i=0;
    while(true){ size_t j=k.find('.',i); std::string part=k.substr(i, j==std::string::npos?std::string::npos:j-i);
        cur=cur->get(part); if(!cur) return "";
        if(j==std::string::npos) break; i=j+1; }
    return cur->tag==Value::Tag::Str? cur->s : "";
}
static long long get_int(const ValuePtr& v, const std::string& k){
    if (!v) return -99999;
    ValuePtr cur=v; size_t i=0;
    while(true){ size_t j=k.find('.',i); std::string part=k.substr(i, j==std::string::npos?std::string::npos:j-i);
        cur=cur->get(part); if(!cur) return -99999;
        if(j==std::string::npos) break; i=j+1; }
    return cur->tag==Value::Tag::Int? cur->i : -99999;
}
static bool has(const ValuePtr& v, const std::string& k){
    ValuePtr cur=v; size_t i=0;
    while(true){ size_t j=k.find('.',i); std::string part=k.substr(i, j==std::string::npos?std::string::npos:j-i);
        cur=cur->get(part); if(!cur) return false;
        if(j==std::string::npos) break; i=j+1; }
    return true;
}

int main(){
    // defaults_are_filled
    { auto v=ok("@contract Server {\n host: str\n port: int default 8080\n tls: bool default true\n}\ndb {\n @is Server\n host: db1.internal\n}");
      CHECK(get_str(v,"db.host")=="db1.internal","db.host");
      CHECK(get_int(v,"db.port")==8080,"db.port default");
      CHECK(v->get("db")->get("tls")->tag==Value::Tag::Bool && v->get("db")->get("tls")->b==true,"db.tls default"); }

    // missing_required_field_fails
    { auto e=err("@contract Server { host: str }\ndb { @is Server }"); CHECK(e.find("host")!=std::string::npos,"missing host reported"); }

    // type_mismatch_fails
    { auto e=err("@contract Server { port: int }\ndb { @is Server\n port: \"not-a-number\" }"); CHECK(e.find("port")!=std::string::npos,"port type mismatch"); }

    // enum_accepts_declared_value
    { auto v=ok("@contract Server { status: enum [ active retired ] }\ndb { @is Server\n status: active }"); CHECK(get_str(v,"db.status")=="active","enum value"); }

    // enum_rejects_undeclared_value
    { auto e=err("@contract Server { status: enum [ active retired ] }\ndb { @is Server\n status: deleted }"); CHECK(e.find("status")!=std::string::npos,"enum reject"); }

    // numeric_bounds_enforced (ok + fail)
    { ok("@contract C { ratio: num min 0 max 1 }\n x { @is C\n ratio: 0.5 }");
      auto e=err("@contract C { ratio: num min 0 max 1 }\n x { @is C\n ratio: 5 }"); CHECK(e.find("ratio")!=std::string::npos,"num bounds"); }

    // array_element_type_checked
    { ok("@contract C { tags: [str] }\n x { @is C\n tags: [ a b c ] }");
      auto e=err("@contract C { tags: [str] }\n x { @is C\n tags: [ a 2 c ] }"); CHECK(e.find("tags")!=std::string::npos,"array elem type"); }

    // optional_field_may_be_absent
    { auto v=ok("@contract C { note: str optional }\n x { @is C }"); CHECK(!has(v,"x.note"),"optional absent"); }

    // unknown_contract_is_error
    { auto e=err("x { @is Nonexistent\n a: 1 }"); CHECK(!e.empty(),"unknown contract error"); }

    // no_contract_behaviour_unchanged
    { auto v=ok("host: db1.internal\nport: 8080\n"); CHECK(get_str(v,"host")=="db1.internal","no-contract host"); CHECK(get_int(v,"port")==8080,"no-contract port"); }

    // contract_definition_not_in_tree
    { auto v=ok("@contract C { a: str }\n x: 1"); CHECK(get_int(v,"x")==1,"contract not in tree x"); CHECK(!has(v,"contract"),"no contract key"); CHECK(!has(v,"C"),"no C key"); }

    // contract_applies_to_multiple_blocks
    { auto v=ok("@contract C { port: int default 80 }\n a { @is C }\n b { @is C\n port: 9090 }"); CHECK(get_int(v,"a.port")==80,"a.port default"); CHECK(get_int(v,"b.port")==9090,"b.port"); }

    // composition_nested_contract_checked
    { auto v=ok("@contract Address {\n city: str\n zip: str optional\n}\n@contract Server {\n host: str\n address: Address\n}\ndb {\n @is Server\n host: db1.internal\n address { city: Beijing }\n}");
      CHECK(get_str(v,"db.host")=="db1.internal","comp host"); CHECK(get_str(v,"db.address.city")=="Beijing","comp nested"); }

    // composition_fills_nested_defaults
    { auto v=ok("@contract Address { city: str  country: str default CN }\n@contract Server { address: Address }\ndb {\n @is Server\n address { city: Shanghai }\n}");
      CHECK(get_str(v,"db.address.country")=="CN","comp nested default"); }

    // composition_rejects_violation_in_nested_contract
    { auto e=err("@contract Address { city: str }\n@contract Server { address: Address }\ndb {\n @is Server\n address { city: 123 }\n}"); CHECK(e.find("city")!=std::string::npos,"nested violation"); }

    // composition_rejects_scalar_where_block_expected
    { auto e=err("@contract Address { city: str }\n@contract Server { address: Address }\ndb {\n @is Server\n address: not-a-block\n}"); CHECK(e.find("address")!=std::string::npos,"scalar where block"); }

    // composition_rejects_unknown_referenced_contract
    { auto e=err("@contract Server { address: Nonexistent }\ndb {\n @is Server\n address { city: x }\n}"); CHECK(!e.empty(),"unknown referenced contract"); }

    // referenced_contract_may_be_defined_later
    { auto v=ok("@contract Server { address: Address }\n@contract Address { city: str }\ndb {\n @is Server\n address { city: Chengdu }\n}"); CHECK(get_str(v,"db.address.city")=="Chengdu","forward ref"); }

    // strict_mode_rejects_undeclared_field
    { auto e=err("@contract Server { host: str }\ndb {\n @is Server\n host: db1.internal\n prot: 5432\n}"); CHECK(e.find("prot")!=std::string::npos,"strict rejects prot"); }

    // loose_mode_allows_undeclared_field
    { auto v=ok("@contract Server loose { host: str }\ndb {\n @is Server\n host: db1.internal\n extra: anything\n}"); CHECK(get_str(v,"db.host")=="db1.internal","loose host"); CHECK(has(v,"db.extra"),"loose extra kept"); }

    // loose_still_validates_declared_fields
    { auto e=err("@contract Server loose { port: int }\ndb {\n @is Server\n port: not-an-int\n extra: 1\n}"); CHECK(e.find("port")!=std::string::npos,"loose still validates"); }

    // enum accepts coerced scalar (int)
    { auto v=ok("@contract C { status: enum [ 0 1 ] }\n x { @is C\n status: 0 }"); CHECK(get_int(v,"x.status")==0,"enum coerced int"); }

    if(failures==0) std::cout<<"ALL CONTRACT TESTS PASSED\n";
    else { std::cout<<failures<<" FAILURES\n"; return 1; }
    return 0;
}
