use candle_core::{Result, Tensor, D};
use candle_nn::{embedding, layer_norm, linear, Dropout, Embedding, LayerNorm, Linear, Module, VarBuilder};

pub struct Attention {
    query: Linear,
    key: Linear,
    value: Linear,
    project: Linear,
    heads: usize,
    scale: f64,
    drop: Dropout,
}

impl Attention {
    pub fn new(dim: usize, heads: usize, drop: f32, variables: VarBuilder) -> Result<Self> {
        Ok(Self {
            query: linear(dim, dim, variables.pp("query"))?,
            key: linear(dim, dim, variables.pp("key"))?,
            value: linear(dim, dim, variables.pp("value"))?,
            project: linear(dim, dim, variables.pp("project"))?,
            heads,
            scale: 1.0 / ((dim / heads) as f64).sqrt(),
            drop: Dropout::new(drop),
        })
    }

    pub fn forward(&self, state: &Tensor, train: bool) -> Result<Tensor> {
        let (batch, seq, dim) = state.dims3()?;
        let size = dim / self.heads;

        let queries = self.query.forward(state)?
            .reshape((batch, seq, self.heads, size))?
            .transpose(1, 2)?
            .contiguous()?;

        let keys = self.key.forward(state)?
            .reshape((batch, seq, self.heads, size))?
            .transpose(1, 2)?
            .contiguous()?;

        let values = self.value.forward(state)?
            .reshape((batch, seq, self.heads, size))?
            .transpose(1, 2)?
            .contiguous()?;

        let transposed = keys.transpose(D::Minus1, D::Minus2)?.contiguous()?;
        let scores = queries.matmul(&transposed)?;
        let scaled = (scores * self.scale)?;

        let mask = Self::mask(seq, state.device())?;
        let masked = scaled.broadcast_add(&mask)?;

        let weights = candle_nn::ops::softmax(&masked, D::Minus1)?;
        let dropped = self.drop.forward(&weights, train)?;
        let context = dropped.matmul(&values)?;
        let merged = context.transpose(1, 2)?.contiguous()?.reshape((batch, seq, dim))?;

        self.project.forward(&merged)
    }

    fn mask(size: usize, device: &candle_core::Device) -> Result<Tensor> {
        let mut values = Vec::new();
        for row in 0..size {
            for col in 0..size {
                if col > row {
                    values.push(f32::NEG_INFINITY);
                } else {
                    values.push(0.0);
                }
            }
        }
        let tensor = Tensor::from_vec(values, (size, size), device)?;
        tensor.unsqueeze(0)?.unsqueeze(0)
    }
}

pub struct Forward {
    inner: Linear,
    outer: Linear,
    drop: Dropout,
}

impl Forward {
    pub fn new(dim: usize, drop: f32, variables: VarBuilder) -> Result<Self> {
        let hidden = dim * 4;
        Ok(Self {
            inner: linear(dim, hidden, variables.pp("inner"))?,
            outer: linear(hidden, dim, variables.pp("outer"))?,
            drop: Dropout::new(drop),
        })
    }

    pub fn forward(&self, state: &Tensor, train: bool) -> Result<Tensor> {
        let hidden = self.inner.forward(state)?;
        let activated = hidden.gelu()?;
        let dropped = self.drop.forward(&activated, train)?;
        self.outer.forward(&dropped)
    }
}

pub struct Network {
    tokens: Embedding,
    positions: Embedding,
    attn: Attention,
    norm_one: LayerNorm,
    feed: Forward,
    norm_two: LayerNorm,
    output: Linear,
    drop: Dropout,
}

impl Network {
    pub fn new(variables: VarBuilder, vocab: usize, dim: usize, heads: usize, limit: usize, drop: f32) -> Result<Self> {
        Ok(Self {
            tokens: embedding(vocab, dim, variables.pp("tokens"))?,
            positions: embedding(limit, dim, variables.pp("positions"))?,
            attn: Attention::new(dim, heads, drop, variables.pp("attn"))?,
            norm_one: layer_norm(dim, 1e-5, variables.pp("norm_one"))?,
            feed: Forward::new(dim, drop, variables.pp("feed"))?,
            norm_two: layer_norm(dim, 1e-5, variables.pp("norm_two"))?,
            output: linear(dim, vocab, variables.pp("output"))?,
            drop: Dropout::new(drop),
        })
    }

    pub fn forward(&self, input: &Tensor, train: bool) -> Result<Tensor> {
        let (_batch, seq) = input.dims2()?;
        let steps = Tensor::arange(0u32, seq as u32, input.device())?.unsqueeze(0)?;

        let words = self.tokens.forward(input)?;
        let places = self.positions.forward(&steps)?;
        let state = words.broadcast_add(&places)?;
        let dropped = self.drop.forward(&state, train)?;

        let first = self.norm_one.forward(&dropped)?;
        let attended = self.attn.forward(&first, train)?;
        let combined = (&dropped + &attended)?;

        let second = self.norm_two.forward(&combined)?;
        let forward = self.feed.forward(&second, train)?;
        let merged = (&combined + &forward)?;

        self.output.forward(&merged)
    }
}
