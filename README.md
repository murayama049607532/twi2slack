# Twi2Slack

Twi2Slack は、公開されている Nitter インスタンスを利用し、疑似的に Twitter-Slack 連携を行う Slack アプリケーションです。

一定間隔ごとにインスタンスの RSS を読み込み、該当ツイートの Twitter へのリンクを設定したチャンネルに送信します。また、ツイートが複数の画像を含む場合は、２枚目以降の画像へのリンクも同時に送信されます。 \
実装上、単一インスタンスに複数のアカウントを指定する場合、複数インスタンスに分散させる場合に比べて、RSS取得間隔が大幅に広がります。


### 登録
URLでの指定、またはアカウントでの指定が可能です。アカウントで指定した場合は、.env ファイルで設定されたデフォルトのインスタンスが使用されます。

`/mock_twitter https://nitter.net/twitterjp/rss`
`/mock_twitter twitterjp`

### 解除
解除はアカウントでのみ指定可能です。

`/mock_twitter remove twitterjp`

