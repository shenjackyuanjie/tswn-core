# 开始

tswn 开始实际上是因为跟群里群友赌气而开始的

这中间的细节我不太想说，主要也有我自己的原因，反正最终，tswn 这个项目开始了

TSWN: tsw namerena

其实 tswn 最开始是作为 namerena-rs 的一部分 存在的, 当然很快我就意识到这个东西应该有自己的地方
很快，tswn也就有了自己的仓库

## 项目概况

项目的核心是 [namerena](https://deepmess.com/zh/namerena/) 这款纯文字对战游戏

这款游戏是由 dart2 编写, 通过 `dart compile js -O2` (你还会在后文中看到这个命令多次) 编译成 js 来在浏览器中运行
[dart官方文档](https://dart.dev/tools/dart-compile#basic-options)

而目标是把这个游戏的核心代码重写成 rust

一方面用来为游戏提速，增加 “开箱” 的效率

另一方面为了增加对游戏的了解，以及可能的对游戏中的某些行为进行修改
