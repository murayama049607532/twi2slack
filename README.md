# Twi2Slack

Twi2Slack は、公開されている Nitter インスタンスを利用し、疑似的に Twitter-Slack 連携を行う Slack アプリケーションです。 \
およそ10分ごとにインスタンスの RSS を読み込み、該当ツイートの Twitter へのリンクを設定したチャンネルに送信します。


### 登録
URLでの指定、またはアカウントでの指定が可能です。アカウントで指定した場合は、.env ファイルで設定されたデフォルトのインスタンスが使用されます。

`/mock_twitter https://nitter.it//twitterjp/rss`
`/mock_twitter twitterjp`

### 解除
`/mock_twitter remove twitterjp`

