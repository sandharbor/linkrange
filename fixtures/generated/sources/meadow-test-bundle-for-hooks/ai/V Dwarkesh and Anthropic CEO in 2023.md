https://youtu.be/Nlkk3glap_U?si=CoW0Qw2Oz6F2wnOK

[[person - Dwarkesh Patel]] and [[person - Dario Amodei]]

Just a test of two blocks hanging together.  This is block 1
...
[[dim - number of parameters -- higher]]
:
This is the second block, which comes after a line with just a colon.
...
[[mechanistic interpretability]]
......
And here's the end annotation that applies to both.

---

At 1:00 scaling: he makes the point that with more parameters it's possible to determine more long-tail correlations between things.
...
[[dim - number of parameters -- higher]]

At 3:00, the scaling is super smooth, and you wouldn't expect to see the loss (or entropy) decrease in such a predictable way outside of something like physics.  The abilities on the other hand are very hard to predict. For example, whether the model will be able to code well or not. It can also be very abrupt like, it can't do it for a long time, and then suddenly it can.
...
[[diff - core metric like training loss vs. a capability that you actually care about]] [[alias - training loss = entropy]] [[training loss]] [[entropy]]

At 4:30 they talk about the possibility that there's a [[circuit]] for doing addition, and that it may grow over time, then get connected to other circuits.  He doesn't understand what leads to that, but it's an area that they are trying to understand better with their work on what he calls [[mechanistic interpretability]]

At 6:00, what would his prediction be if [[con - scaling plateaus before reaching human level intelligence]]? [[diff - fundamental theory vs. practical issue]] ... [[practical issue - running out of data]]
...
[[scaling problems]]

At 6:30 perhaps there is another change necessary in architecture?  For example, it is very important to be able to have [[self-attention]] to information significantly far back in the way that [[transformers]] can, but [[recurrent neural networks - RNN]]'s can't.
...
I wonder what he would think about recent innovations like [[project - mamba]] and [[company - Google]]'s [[con - one million token context window]] [[moc - architectures]]

At 7:35 the things that the models can't do isn't different and kind from the things that they can do. In the past it was true that the model just simply couldn't do certain things. For example, they couldn't code, but now there's no thing that needs to be totally different. X [[con - we don't need completely new capabilities]]

At 9:00 Dwarkesh asks if it turns out that [[next token prediction]] it's not the [[loss function]] to get us there, what would the alternative candidate? He says [[reinforcement learning - RL]] against an objective, or [[reinforcement learning with human feedback - RLHF]]

At 10:15 when talking about [[practical issue - running out of data]] again, he says it's probably not a problem because there's many sources of data in the world, and also there are ways of generating data.
...
[[synthetic data]]

At 12:00 he was given the task of trying to improve speech recognition and just ran a set of simple experiments. He added layers, saw how long it took for training to cause overfitting, etc. Really just blindly experimenting.

At 14:50 Dwarkesh asked how he saw the generalization of these capabilities when he was only working on speech recognition, and also how come other people besides, [[person - Ilya Sutskever]] didn't see it.  He said that part of the reason he saw it is that between 2014 and 2017 he tried it for other stuff as well and it also worked on other stuff. Then he talked about robotics, and how it was seen as a counter example, but the problem was actually that [[practical issue - it was hard to get data for robotics]]

At 15:45 one reason for [[next token prediction]] was that it allowed an easy mechanism for [[self-supervised learning - SSL]]

At 19:10 how do you square the idea that these models perform at expert levels on [[performance benchmarks]] but are clearly not general [[true AGI]] yet?

At 19:45 he says that he is very [[empiricist]] because he is so often surprised by observed phenomena. X [[diff - discovered vs. invented]] [[diff - empiricist vs. theoretician]]

At 23 minutes Dwarkesh asks how many years do you think that the models will be superhuman at [[economically valuable]] tasks [[narrow ASI]] before [[true AGI]]?

At 26 minutes, what's the economic value of [[project - Claude]] at [[company - Anthropic]] right now?  He says, basically an intern at most levels, but in some places more than that. X [[con - in most places, AI is about as good as an intern]]

At 32 Dwarkesh asks whether the [[diminishing returns]] of scaling will dominate, or whether [[con - AI can speed up AI research]]. Dario suggests that the primary thing that will dominate is the amount of money and people who are flooding the field. X [[exponential progress]]

At 36 Dwarkesh asks, why haven't we seen any scientific innovation yet from these models despite the fact that they have an extremely broad set of knowledge.  Dario says that these models know a lot of things, but they just don't have the level of skill to put them together yet.  But he thinks that's probably coming shortly.

At 42:30 people don't just judge the post, they also judge the organization.

At 43:10 he talks about leaps in functionality as [[slang - groks]].  So, for example, when it learned how to do math, or how to code, or the next thing

At 43:50 architectural innovations that improve training are called [[compute multiplier - CM]] because they're like having more compute x [[architectural breakthrough]] [[moc - architectures]]

At 49:50 [[mechanistic interpretability]] is more like an x-ray than modifying the model.  He also talks about how [[con - you should never train for interpretability]].  He talks about how this is similar to concerns about [[leakage]] in [[cross-validation]].

At 51:00 he talks about the [[very hard challenge]] of testing for alignment without having that test be something that is also built into the objective of the model.  He talks about how there are a lot of stupid ways to do this where you fool yourself.

At 54:00, there is a test in an MRI that can determine with better than random chance that you are a psychopath. This is the kind of test that they would want to be able to do on the internals of a model such that the exhibited behavior of charming and very goal oriented on the outside, wouldn't trick you

At 57:00 Dwarkesh mentions that one of [[company - Anthropic]]'s ideas is that to really work on safety and alignment you have to be working on [[frontier models]] which means you have to have the best capabilities, and it means that you have to be basically as good as the big guys.

1:00:50 [[con - using bigger models to interpret smaller models]] [[dim - model size]] this is one reason why you need the [[frontier models]]

At 1:04:00 do not only need to solve for alignment, but you also need to solve for potential [[misuse]], like where it is aligned with a single government desires, but that's not good for the world, or a single bad actor x [[risks]]

At 1:05:50 it's so powerful that somebody needs to be in charge of managing it, but it's not entirely clear who. The UN maybe is not the right body it should include the role of democratically, elected people, governmental bodies, etc..

At 1:07:00 they talk about the long-term benefit trust which is a body that governs anthropic and will overtime steer the appointment of people to the board. X As we've seen with [[The OpenAI leadership drama and board turnover]] [[con - incentives can be so powerful that they override explicit governing structures]]

At 1:11:20 why do you think china has under performed in AI scaling?

At 1:13:00 one of the concerns, and one of the reasons for pushing for so much cyber security is that China could end up with the blueprints and relatively quickly catch up to the frontier.  Earlier in the conversation, he talks about how they have structured their company into cells, where very few people know all the secrets to try to combat some of the problems. X  [[con - AI secret technique proliferation]]

At 1:18:00 the idea of [[aligned by default]] or [[doomed by default]]

At 1:24:00 he asked the question about beyond [[mechanistic interpretability]], what are the other methods they are looking at?  One thing he mentions in passing is [[constitutional AI]]