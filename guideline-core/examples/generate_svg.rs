use guideline_core::{parse, render};

fn main() {
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

note over User,CSBrowser:・session_id:認証シーケンス開始前からWebViewとBFF間のセション

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
note over BFF:channel≠"Native"の場合は400を返却し処理を終了

BFF->Redis:key=code,\nvalue=session_id,\nnative_state,ttl=60s

opt Redisへのデータ保存失敗
    BFF-->Native:500
    Native->Native:show dialog
    Native->Webview:Foreground\nshow security setting
end

BFF-->Native:正常応答

Native->CSBrowser:open

CSBrowser->BFF:NativeApp用出金認証\nログインエンドポイント\nquerystring:code

BFF->Redis:Get code

opt code取得失敗
    BFF->CSBrowser:redirect to error screen
    CSBrowser->CSBrowser:show error screen\n再実施を案内する
end

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

User->CSBrowser:出金パスワード入力&submit

CSBrowser->idP:Submit
idP->idP:verify

opt auth0セッション切れ
    idP-->CSBrowser:redirect to initiate_login_uri=専用エラーページのURL
    CSBrowser->CSBrowser:エラーページ表示
end

idP-->CSBrowser:redirect to BFF

CSBrowser->BFF:callback

BFF->Redis:session取得(key:session_id_csb)
BFF->BFF:verify state

opt stateの検証成功
    BFF->Redis:delete key=session_id_csb
end

opt ユーザーが出金認証をキャンセル
    BFF-->CSBrowser:redirect to cancel screen\nset-cookie:session_id_csb削除
    CSBrowser->CSBrowser:show cancel screen\nキャンセルの旨を表示
end

BFF->Redis:session取得(key:session_id)\nsession_idはsession_id_csbに紐つく\n出金認証用セッションから取得

BFF->idP:token request
idP-->BFF:

BFF->BFF:verify idToken

note over BFF:通常のidToken検証に加えて、下記を実施\n・IDトークンから取得した口座番号がsession取得(key:session_id)\nによって取得したaccountNoと一致すること\n・出金認証のコネクションと一致\n・auth_timeのチェック

opt session取得・state検証・idToken検証失敗
    BFF->CSBrowser:redirect to error screen
    CSBrowser->CSBrowser:show error screen\n再実施を案内する
end

BFF->BFF:セキュリティ設定画面用セッションID生成\n(security_setting_session_id)

BFF->Redis:update key=session_id,\nvalue=security_setting_session_id

BFF->Redis:store key=security_setting_session_id,\nvalue=access_token,ttl=expire_in

BFF->CSBrowser:redict to 出金認証完了ページ\nset-cookie:session_id_csb削除\nquery string native_state

alt UniversalLink＆AppLink発火
    Native->Native:native_state検証
    opt 検証成功
        Native->Native:native_state破棄
        Native->Webview:foreground
    end
else UniversalLink＆AppLink未発火
    CSBrowser->CSBrowser:出金認証完了ページ
    CSBrowser-->Native:Custom url scheme
    Native->Native:native_state検証
    opt 検証成功
        Native->Native:native_state破棄
        Native->Webview:foreground
    end
end

Webview->Webview:reload

Webview->BFF:セキュリティ設定画面セッションチェック
BFF->Redis:session取得(key:session_id)
BFF->BFF:xsrf-token検証

opt ログインセッション取得失敗/xsrf-token検証失敗
    BFF-->Webview:401
    Webview->Webview:show dialog\nsession expired
end

Redis-->BFF:value=security_setting_session_id
BFF->Redis:Get Sequrity Setting Session

opt セキュリティ設定画面用セッション取得失敗
    BFF-->Webview:401(error=invalid_grant)
    Webview->Webview:セキュリティ設定画面\n(認証前)
end

Redis-->BFF:access_token

BFF-->Webview:正常応答

note over User,idP:セキュリティ設定画面(認証後)
"#;

    match parse(input) {
        Ok(diagram) => {
            let svg = render(&diagram);
            println!("{}", svg);
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }
}
