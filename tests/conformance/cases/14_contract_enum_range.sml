# 钉住：契约的**枚举取值**与**数值区间**（含端点的 min/max）。
# 期望：s = { host: h1, port: 5432（默认）, status: active, weight: 80 }。
# 这里只放**合法**值 —— 非法值应当报错，属另一类（负向）用例，避免把
# 「校验失败」和「值树不一致」混在一个格子里。
@contract S {
    host: str
    port: int default 5432
    status: enum [ active standby retired ]
    weight: num min 0 max 100
}

s { @is S  host: h1  status: active  weight: 80 }
