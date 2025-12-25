//! Real-world test cases

use guideline_core::{parse, render};

#[test]
fn test_security_setting_diagram() {
    let input = r#"
title security-setting-withdrawal-auth-native

actor User
participant Native
participant Webview
participant CSBrowser
participant BFF
participant Redis
participant idP

note over CSBrowser:sfsafariviewcontroller\nchrome custom tabs

note over User,idP:セキュリティ設定画面(認証前)

User->Webview:tap [変更する]

Native->Native:HOOK\ncode,native_state生成\nsession_id,xsrf-token取得\n(webviewの設定値を取得)

Native->BFF:POST Code登録エンドポイント\ncookie:session_id\nrequest-header(x-xsrf-token):xsrf-token\nrequest body:native_state,code

BFF->Redis:session取得(key:session_id)
Redis-->BFF:channel,accountNo,sub,\nxsrfToken,token
BFF->BFF:verify xsrf-token

opt session取得失敗・xsrf-token検証失敗
    BFF-->Native:401
    Native->Native:show dialog\nsession expired
    Native->Webview:close
    Native->Native:show native\nlogin screen
end

BFF->BFF:verify channel

BFF->Redis:key=code,\nvalue=session_id,\nnative_state,ttl=60s

BFF-->Native:正常応答

Native->CSBrowser:open

CSBrowser->BFF:NativeApp用出金認証\nログインエンドポイント\nquerystring:code

BFF->Redis:Get code

Redis-->BFF:value=session_id

BFF->Redis:Delete code

BFF->Redis:session取得(key:session_id)
Redis-->BFF:

BFF->BFF:セッションチェック\n(セッションが取得できたかのみ)

BFF->BFF:認可リクエスト生成

BFF->BFF:CSBrowserでの出金認証用\nSessionID(session_id_csb)生成

BFF->Redis:update session(key:session_id)\nttl更新(3hours)

BFF->Redis:store session(key:session_id_csb)\nstate,nonce,codeVerifier,\nnative_state,session_id

BFF->CSBrowser:Redirect to Auth0\n set-cookie:session_id_csb

CSBrowser->idP:Authz Request

idP-->CSBrowser:出金認証ログイン画面

User->CSBrowser:出金パスワード入力

CSBrowser->idP:Submit
idP->idP:verify

idP-->CSBrowser:redirect to BFF

CSBrowser->BFF:callback

BFF->Redis:session取得(key:session_id_csb)
BFF->BFF:verify state

BFF->idP:token request
idP-->BFF:

BFF->BFF:verify idToken

BFF->BFF:セキュリティ設定画面用セッションID生成\n(security_setting_session_id)

BFF->Redis:update key=session_id,\nvalue=security_setting_session_id

BFF->Redis:store key=security_setting_session_id,\nvalue=access_token,ttl=expire_in

alt UniversalLink発火
    Native->Native:native_state検証
    Native->Webview:foreground
else AppLink未発火
    CSBrowser->CSBrowser:出金認証完了ページ
    Native->Native:native_state検証
    Native->Webview:foreground
end

Webview->Webview:reload

Webview->BFF:セキュリティ設定画面セッションチェック
BFF->Redis:session取得(key:session_id)
BFF->BFF:xsrf-token検証

Redis-->BFF:value=security_setting_session_id
BFF->Redis:Get Sequrity Setting Session

Redis-->BFF:access_token

BFF-->Webview:正常応答
"#;

    // Parse should succeed
    let diagram = parse(input).expect("Failed to parse diagram");

    // Check title
    assert_eq!(
        diagram.title,
        Some("security-setting-withdrawal-auth-native".to_string())
    );

    // Check participants
    let participants = diagram.participants();
    assert!(participants.len() >= 7);

    // Render should succeed
    let svg = render(&diagram);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("User"));
    assert!(svg.contains("BFF"));
    assert!(svg.contains("Redis"));

    // Check that notes are rendered
    assert!(svg.contains("sfsafariviewcontroller"));

    // Check that blocks are rendered
    assert!(svg.contains("opt"));
    assert!(svg.contains("alt"));

    println!("SVG length: {} bytes", svg.len());
}

#[test]
fn test_simple_auth_flow() {
    let input = r#"
title Simple Auth

actor User
participant App
participant Server

User->App: Login
App->Server: POST /auth
Server-->App: token
App-->User: Success

note over App: Token stored

opt Remember me
    App->App: Save to storage
end
"#;

    let diagram = parse(input).unwrap();
    let svg = render(&diagram);

    assert!(svg.contains("<svg"));
    assert!(svg.contains("Simple Auth"));
    assert!(svg.contains("Login"));
}
