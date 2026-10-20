//! 巨懂车集成：列表接口与导出链路的客户端、解析。
//! spike 结论（issue #3）：无签名参数；认证 = 整罐 cookie + content-type + UA + app-id 头。

pub mod parse;
